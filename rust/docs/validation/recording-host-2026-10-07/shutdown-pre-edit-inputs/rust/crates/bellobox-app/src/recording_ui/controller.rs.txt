//! Logical UI state and physical retirement are independent. Closing releases the
//! handle immediately; the native worker owns terminal cleanup and admission.
use bello_platform::recording::{
    FinalizedRecording, RecordingControl, RecordingEvent, RecordingHandle,
};
use bellobox_core::recording::session::{Phase, Session, Ticket};

#[derive(Default)]
pub(super) struct Controller {
    pub session: Session,
    pub status: String,
    active: Option<(Ticket, RecordingHandle)>,
    retiring: Option<(Ticket, RecordingControl)>,
    completed: Option<FinalizedRecording>,
}
impl Controller {
    pub fn start(
        &mut self,
        factory: impl FnOnce() -> Result<RecordingHandle, bello_platform::recording::RecordingError>,
    ) {
        let Some(ticket) = self.session.begin() else {
            return;
        };
        self.completed = None;
        self.status = "Starting…".into();
        match factory() {
            Ok(handle) => self.active = Some((ticket, handle)),
            Err(error) => {
                self.session.completed(ticket, false);
                self.session.drained(ticket);
                self.status = error.to_string();
            }
        }
    }
    pub fn stop(&mut self) {
        if self.session.stop().is_some() {
            if let Some((_, handle)) = &self.active {
                handle.stop();
            }
            self.status = "Finishing movie…".into();
        }
    }
    pub fn cancel(&mut self) {
        self.session.cancel();
        let publication_claimed = self.active.as_ref().is_some_and(|(_, handle)| {
            handle.cancel() == bello_platform::recording::CancelOutcome::PublicationClaimed
        });
        self.completed = None;
        self.status = if publication_claimed { "Movie publication already started; waiting for completion. Any completed movie is kept." } else if self.session.busy() {
            "Cancelling; waiting for the writer to finish cleanup…"
        } else {
            "Recording cancelled."
        }
        .into();
    }
    pub fn close(&mut self) {
        self.cancel();
        self.session.close();
        if let Some((ticket, handle)) = self.active.take() {
            self.retiring = Some((ticket, handle.control()));
            // Platform Drop changes atomics only. Its native owner, not a UI
            // poll or this retained entity, retires the terminal payload.
            drop(handle);
        }
    }
    #[cfg(all(test, feature = "recording-fixtures"))]
    pub fn control_for_test(&self) -> Option<RecordingControl> {
        self.active
            .as_ref()
            .map(|(_, handle)| handle.control())
            .or_else(|| self.retiring.as_ref().map(|(_, control)| control.clone()))
    }
    pub fn poll(&mut self) -> Option<FinalizedRecording> {
        if let Some((ticket, control)) = &self.retiring {
            if control.is_drained() {
                self.session.drained(*ticket);
                self.retiring = None;
            }
            return None;
        }
        let (ticket, handle) = self.active.as_ref()?;
        let ticket = *ticket;
        // Observe drain before draining the channel. A terminal event is sent
        // before physical retirement; checking in the reverse order can miss it.
        let drained = handle.is_drained();
        while let Some(event) = handle.try_event() {
            match event {
                RecordingEvent::Started if self.session.started(ticket) => {
                    if self.session.phase() == Phase::Recording {
                        self.status = "Recording. Choose Stop to finish the movie.".into();
                    }
                }
                RecordingEvent::Finalized(movie) => {
                    if self.session.completed(ticket, true) {
                        self.completed = Some(movie);
                    }
                }
                RecordingEvent::Failed(error) => {
                    if self.session.completed(ticket, false) {
                        self.status = error.to_string();
                    }
                }
                RecordingEvent::Cancelled if self.session.completed(ticket, false) => {
                    self.status = "Recording cancelled.".into();
                }
                _ => {}
            }
        }
        if !drained {
            return None;
        }
        self.active = None;
        self.session.drained(ticket);
        if self.session.phase() == Phase::Reviewing {
            self.completed.take()
        } else {
            self.completed = None;
            None
        }
    }
}
impl Drop for Controller {
    fn drop(&mut self) {
        self.close();
    }
}
