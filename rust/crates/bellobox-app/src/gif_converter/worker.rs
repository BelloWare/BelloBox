//! One active physical worker and one replaceable pending request per converter.
//! Taking a request does not release admission: completion must explicitly retire
//! its exact ticket. Metadata callbacks/native teardown complete inside run().
use super::model::{Inspection, SelectedSource, Source, SourceFrame, Work};
use bello_platform::movie::MovieCancellation;
use bellobox_core::recording::gif::{ExportControl, GifError, GifExportResult};

pub(super) enum Request {
    Inspect {
        generation: u64,
        source: Source,
        cancel: MovieCancellation,
    },
    Seek {
        generation: u64,
        revision: u64,
        source: SelectedSource,
        seconds: f64,
        cancel: MovieCancellation,
    },
    Export(Work),
}
pub(super) enum Response {
    Inspect {
        generation: u64,
        result: Result<Inspection, String>,
    },
    Seek {
        generation: u64,
        revision: u64,
        result: Result<SourceFrame, String>,
    },
    Export {
        generation: u64,
        result: Result<GifExportResult, GifError>,
    },
}
#[derive(Clone)]
struct Cancellation {
    movie: MovieCancellation,
    export: Option<ExportControl>,
}
impl Cancellation {
    fn cancel(&self) {
        self.movie.cancel();
        if let Some(export) = &self.export {
            let _ = export.cancel();
        }
    }
}
impl Request {
    fn cancellation(&self) -> Cancellation {
        match self {
            Self::Inspect { cancel, .. } | Self::Seek { cancel, .. } => Cancellation {
                movie: cancel.clone(),
                export: None,
            },
            Self::Export(work) => Cancellation {
                movie: work.movie_cancel.clone(),
                export: Some(work.control.clone()),
            },
        }
    }
    pub fn aborted_response(&self) -> Response {
        let message = "Media worker stopped unexpectedly. Try again.".to_string();
        match self {
            Self::Inspect { generation, .. } => Response::Inspect {
                generation: *generation,
                result: Err(message),
            },
            Self::Seek {
                generation,
                revision,
                ..
            } => Response::Seek {
                generation: *generation,
                revision: *revision,
                result: Err(message),
            },
            Self::Export(work) => Response::Export {
                generation: work.generation,
                result: Err(GifError::Source(message)),
            },
        }
    }
    pub fn identity(&self) -> (u64, Option<u64>) {
        match self {
            Self::Inspect { generation, .. } => (*generation, None),
            Self::Seek {
                generation,
                revision,
                ..
            } => (*generation, Some(*revision)),
            Self::Export(work) => (work.generation, None),
        }
    }
    pub fn cancel_on_shutdown(&self) -> impl Fn() + Send + Sync + 'static {
        let cancel = self.cancellation();
        move || cancel.cancel()
    }
    pub fn run(self) -> Response {
        match self {
            Self::Inspect {
                generation,
                source,
                cancel,
            } => Response::Inspect {
                generation,
                result: source.inspect(cancel),
            },
            Self::Seek {
                generation,
                revision,
                source,
                seconds,
                cancel,
            } => Response::Seek {
                generation,
                revision,
                result: source.seek(seconds, cancel),
            },
            Self::Export(work) => Response::Export {
                generation: work.generation,
                result: work.run(),
            },
        }
    }
}
#[derive(Default)]
pub(super) struct Controller {
    active: Option<(u64, Cancellation)>,
    pending: Option<Request>,
    next: u64,
    closed: bool,
}
impl Controller {
    pub fn submit(&mut self, request: Request) {
        if self.closed {
            request.cancellation().cancel();
            return;
        }
        if let Some((_, cancel)) = &self.active {
            cancel.cancel();
        }
        if let Some(old) = self.pending.replace(request) {
            old.cancellation().cancel();
        }
    }
    pub fn start(&mut self) -> Option<(u64, Request)> {
        if self.closed || self.active.is_some() {
            return None;
        }
        let request = self.pending.take()?;
        self.next += 1;
        self.active = Some((self.next, request.cancellation()));
        Some((self.next, request))
    }
    pub fn retire(&mut self, ticket: u64) {
        if self.active.as_ref().is_some_and(|(id, _)| *id == ticket) {
            self.active = None;
        }
    }
    pub fn close(&mut self) {
        self.closed = true;
        if let Some((_, cancel)) = &self.active {
            cancel.cancel();
        }
        if let Some(request) = self.pending.take() {
            request.cancellation().cancel();
        }
    }
}
impl Drop for Controller {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn inspect(generation: u64) -> Request {
        Request::Inspect {
            generation,
            source: Source::Synthetic,
            cancel: Default::default(),
        }
    }
    #[test]
    fn latest_request_waits_for_physical_retirement_not_logical_cancel() {
        let mut controller = Controller::default();
        controller.submit(inspect(1));
        let (first, request) = controller.start().unwrap();
        let cancellation = request.cancellation();
        for generation in 2..1000 {
            controller.submit(inspect(generation));
        }
        assert!(cancellation.movie.is_cancelled());
        assert!(controller.start().is_none());
        controller.retire(first + 1);
        assert!(
            controller.start().is_none(),
            "a stale completion is not physical retirement"
        );
        assert!(matches!(
            request.run(),
            Response::Inspect { result: Err(_), .. }
        ));
        controller.retire(first);
        let (last, request) = controller.start().unwrap();
        assert!(matches!(
            request.run(),
            Response::Inspect {
                generation: 999,
                result: Ok(_),
                ..
            }
        ));
        controller.retire(last);
        assert!(controller.start().is_none());
    }
    #[test]
    fn close_drops_pending_and_keeps_active_cancelled_until_retired() {
        let mut controller = Controller::default();
        controller.submit(inspect(1));
        let (ticket, active) = controller.start().unwrap();
        controller.submit(inspect(2));
        controller.close();
        assert!(controller.pending.is_none());
        assert!(active.cancellation().movie.is_cancelled());
        assert!(controller.active.is_some());
        controller.retire(ticket);
        controller.submit(inspect(3));
        assert!(controller.start().is_none());
    }
}
