use bello_platform::movie::{generated_movie, GeneratedOrientation, MovieAsset, MovieError, MovieReadRange, MovieReader};
use bellobox_core::recording::gif::{self as core, DisplayRgbaFrame, ExportControl, GifError, GifExportOptions, GifExportPlan, GifSourceInfo, ReplacePolicy, SequentialFrameSource};
use std::{collections::VecDeque, fs, io::Cursor, path::{Path, PathBuf}};

type Raw = (f64, u32, u32, Vec<u8>);
struct Native { reader: MovieReader, frames: Vec<Raw> }
fn movie_error(e: MovieError) -> GifError {
    if e == MovieError::Cancelled { GifError::Cancelled } else { GifError::Source(e.to_string()) }
}
impl SequentialFrameSource for Native {
    fn next_frame(&mut self, c: &ExportControl) -> Result<Option<DisplayRgbaFrame>, GifError> {
        let frame = self.reader.next_frame(|| c.check_active().is_err()).map_err(movie_error)?;
        frame.map(|f| {
            let p=f.into_parts();self.frames.push(p.clone());
            DisplayRgbaFrame::new(p.0,p.1,p.2,p.3)
        }).transpose()
    }
    fn finish(&mut self,c:&ExportControl)->Result<(),GifError>{
        self.reader.finish(||c.check_active().is_err()).map_err(movie_error)
    }
}
struct Replay(VecDeque<Raw>);
impl SequentialFrameSource for Replay {
    fn next_frame(&mut self,c:&ExportControl)->Result<Option<DisplayRgbaFrame>,GifError>{
        c.check_active()?;
        self.0.pop_front().map(|p| DisplayRgbaFrame::new(p.0,p.1,p.2,p.3)).transpose()
    }
}
fn inspect(bytes:&[u8],strict:bool){
    let mut opts=::gif::DecodeOptions::new();
    opts.set_color_output(::gif::ColorOutput::RGBA);
    opts.check_frame_consistency(true);
    opts.check_lzw_end_code(strict);
    opts.set_memory_limit(::gif::MemoryLimit::Bytes((1080*1080*4).try_into().unwrap()));
    let mut decoder=match opts.read_info(Cursor::new(bytes)){
        Ok(d)=>d,Err(e)=>{println!("strict={strict} header_error={e:?}");return;}
    };
    let mut count=0;
    loop { match decoder.read_next_frame(){
        Ok(Some(f))=>{println!("strict={strict} frame={count} size={}x{} delay={} rgba={} transparent={:?}",f.width,f.height,f.delay,f.buffer.len(),f.transparent);count+=1;}
        Ok(None)=>{println!("strict={strict} complete frames={count}");break;}
        Err(e)=>{println!("strict={strict} decode_error_after={count}: {e:?}");break;}
    }}
}
fn export<S:SequentialFrameSource>(source:&mut S,plan:&GifExportPlan,input:Option<&Path>,directory:&Path){
    fs::create_dir(directory).unwrap();
    let target=directory.join("output.gif");let mut observed:Option<Vec<u8>>=None;
    let control=ExportControl::default();
    let result=core::export_gif(source,plan,input,&target,ReplacePolicy::RefuseExisting,&control,|written,planned|{
        println!("progress={written}/{planned}");
        if written==planned{
            let stage=fs::read_dir(directory).unwrap().map(|e|e.unwrap().path()).find(|p|p.file_name().unwrap().to_string_lossy().starts_with(".BelloBox-export-")).unwrap();
            observed=Some(fs::read(stage).unwrap());
        }
    });
    println!("export_dir={} result={result:?} control={:?}",directory.display(),control.status());
    let stages=fs::read_dir(directory).unwrap().filter(|e|e.as_ref().unwrap().file_name().to_string_lossy().starts_with(".BelloBox-export-")).count();
    println!("stage_leftovers={stages} output_exists={}",target.exists());
    assert_eq!(stages,0);
    if let Some(bytes)=observed{
        fs::write(directory.join("observed-before-trailer.bin"),&bytes).unwrap();
        // Separate diagnostic artifact only. Never change the exporter's stage.
        // This assumes into_inner writes a single trailer; passing exports check it.
        let mut reconstructed=bytes;reconstructed.push(0x3b);
        fs::write(directory.join("diagnostic-appended-trailer.gif"),&reconstructed).unwrap();
        if result.is_ok(){assert_eq!(reconstructed,fs::read(target).unwrap());println!("diagnostic reconstruction equals published bytes");}
        inspect(&reconstructed,true);
        inspect(&reconstructed,false);
    }
}
fn main(){
    let root=PathBuf::from(std::env::args().nth(1).unwrap());fs::create_dir(&root).unwrap();
    for orientation in [GeneratedOrientation::Landscape,GeneratedOrientation::Portrait,GeneratedOrientation::Mirrored]{
        let selected=generated_movie(orientation).unwrap();
        assert!(matches!(MovieAsset::open(selected.path(),Default::default()),Err(MovieError::Unavailable)));
        let original=fs::read(selected.path()).unwrap();
        let name=format!("{orientation:?}");let fixture_dir=root.join(&name);fs::create_dir(&fixture_dir).unwrap();
        fs::write(fixture_dir.join("generated.mov"),&original).unwrap();
        for loops in [false,true]{
            let asset=MovieAsset::open_selected(selected.clone(),Default::default()).unwrap();
            let info=asset.info();
            let plan=GifExportPlan::make(GifSourceInfo{duration:info.duration,display_width:info.display_width,display_height:info.display_height,nominal_frame_rate:info.nominal_frame_rate},
                &GifExportOptions{frames_per_second:10,max_width:320,loops,trim_start:0.1,trim_end:Some(0.251)}).unwrap();
            let range=MovieReadRange::new(plan.start(),plan.end(),plan.frame_delay()).unwrap();
            let mut source=Native{reader:asset.reader(range).unwrap(),frames:Vec::new()};
            println!("CASE {name} loops={loops} info={info:?} plan={plan:?}");
            export(&mut source,&plan,Some(selected.path()),&fixture_dir.join(format!("native-{loops}")));
            for (index,p) in source.frames.iter().enumerate(){
                fs::write(fixture_dir.join(format!("native-{loops}-frame-{index}.rgba")),&p.3).unwrap();
                println!("native_sample={index} pts={} size={}x{} rgba={}",p.0,p.1,p.2,p.3.len());
            }
            let frames=std::mem::take(&mut source.frames);drop(source);
            export(&mut Replay(frames.into()),&plan,None,&fixture_dir.join(format!("replay-{loops}")));
        }
        selected.verify().unwrap();assert_eq!(fs::read(selected.path()).unwrap(),original);
    }
}

