use std::{fs::File,io::BufReader};
fn main(){
 let args:Vec<String>=std::env::args().collect();let mode=&args[1];let path=&args[2];
 let mut o=gif::DecodeOptions::new();o.set_color_output(gif::ColorOutput::RGBA);o.check_frame_consistency(true);o.check_lzw_end_code(true);
 o.set_memory_limit(gif::MemoryLimit::Bytes((1080*1080*4).try_into().unwrap()));
 let mut d=o.read_info(BufReader::new(File::open(path).unwrap())).unwrap();
 let mut count=0;
 loop {
  if mode=="legacy"{
   match d.read_next_frame(){Ok(Some(f))=>{count+=1;println!("frame{count} {}x{} delay{} rgba{}",f.width,f.height,f.delay,f.buffer.len());},Ok(None)=>{println!("complete {count}");break;},Err(e)=>{println!("error after{count}: {e:?}");break;}}
  }else{
   let f=match d.next_frame_info(){Ok(Some(f))=>f.clone(),Ok(None)=>{println!("complete {count}");break;},Err(e)=>{println!("metadata error after{count}: {e:?}");break;}};
   let mut rgba=vec![0;usize::from(f.width)*usize::from(f.height)*4];
   d.read_into_buffer(&mut rgba).unwrap();
   let extra=d.fill_buffer(&mut[0;4]).unwrap();assert!(!extra,"extra decoded pixel");
   count+=1;println!("frame{count} {}x{} delay{} rgba{} strict-drain-extra={extra}",f.width,f.height,f.delay,rgba.len());
  }
 }
}

