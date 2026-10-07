mod clock_preview_session;
mod desktop;
mod gif_converter;
mod home;
mod image_disposal;
mod launcher_clock_ui;
mod launcher_ui;
mod recording_ui;
mod screenshot_color;
mod screenshot_ui;
mod session;
mod shutdown;
mod settings_ui;
mod snippet_library;
mod theme;
mod tool_controls;
mod transport;
mod world_clock_ui;
use bellobox_core::{
    clock::Planner,
    settings::{Settings, config_dir},
    text,
};
use std::{env, path::PathBuf};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if !args.is_empty() {
        if let Err(error) = cli(&args) {
            eprintln!("BelloBox: {error}");
            std::process::exit(1);
        }
        return;
    }
    desktop::run();
}
fn option(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|v| v == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}
fn cli(args: &[String]) -> Result<(), String> {
    match args[0].as_str() {
        "--help" | "-h" => println!(
            "BelloBox Rust preview\n\nNo arguments: native GPUI app\n--smoke: validate local domain engines and capabilities\n--tool ID --input TEXT [--second TEXT]: local utility\n--qr-save FILE.png|FILE.svg --input TEXT: export a real QR code\n--world-clock [--at ISO_OR_UNIX]: planner summary\n--capture FILE.png: explicit full-screen capture (no overwrite)\n--ocr FILE.png: local OCR\n--selection: explicit conservative macOS AX selection\n--ai --input TEXT --instruction TEXT: send explicitly to configured provider\n\nAI: BELLOBOX_AI_ENDPOINT, BELLOBOX_AI_MODEL, BELLOBOX_AI_KEY, BELLOBOX_AI_PROVIDER=openai|responses|anthropic\nSettings: BELLOBOX_CONFIG_DIR overrides the isolated Rust directory.\nThe Swift application and preferences are preserved."
        ),
        "--smoke" => {
            assert_eq!(
                text::case("helloWorld", text::CaseStyle::Snake),
                "hello_world"
            );
            bellobox_core::qr::svg("BelloBox")?;
            let p = Planner::default();
            assert!(p.summary().contains("UTC"));
            let status = bello_platform::Platform::new().status();
            println!("BelloBox domain smoke PASS\n{}\n{status:#?}", p.summary());
        }
        "--tool" => {
            let id = args.get(1).ok_or("Missing tool ID")?;
            println!(
                "{}",
                execute(
                    id,
                    &option(args, "--input").unwrap_or_default(),
                    &option(args, "--second").unwrap_or_default()
                )?
            );
        }
        "--qr-save" => {
            let path = PathBuf::from(args.get(1).ok_or("Missing SVG path")?);
            let input = option(args, "--input").unwrap_or_default();
            let data = if path.extension().is_some_and(|e| e == "png") {
                bellobox_core::qr::png(&input)?
            } else {
                bellobox_core::qr::svg(&input)?.into_bytes()
            };
            save_new(&path, &data)?;
            println!("Saved {}", path.display());
        }
        "--world-clock" => {
            let mut p = Planner::default();
            if let Some(at) = option(args, "--at") {
                p.set_instant(bellobox_core::clock::parse_instant(&at)?)?;
            }
            println!("{}", p.summary());
        }
        "--capture" => {
            let path = PathBuf::from(args.get(1).ok_or("Missing PNG path")?);
            let result = bello_platform::Platform::new()
                .capture_screenshot(&path)
                .map_err(|e| e.to_string())?;
            println!(
                "Captured {} × {} to {}",
                result.width,
                result.height,
                result.path.display()
            );
        }
        "--selection" => {
            #[cfg(target_os = "macos")]
            {
                println!(
                    "{}",
                    bello_platform::macos_native::read_selected_text()
                        .map_err(|e| e.to_string())?
                        .text
                );
            }
            #[cfg(not(target_os = "macos"))]
            {
                return Err("Native selection reading is implemented only for macOS; no clipboard fallback is performed.".into());
            }
        }
        "--ocr" => {
            let path = PathBuf::from(args.get(1).ok_or("Missing image path")?);
            println!(
                "{}",
                bello_platform::Platform::new()
                    .recognize_text(&path, None)
                    .map_err(|e| e.to_string())?
            );
        }
        "--ai" => {
            let config = transport::config_from_env()?;
            let key = env::var("BELLOBOX_AI_KEY").unwrap_or_default();
            let request = bellobox_core::ai::request(
                &config,
                &key,
                &option(args, "--instruction").unwrap_or_default(),
                &option(args, "--input").unwrap_or_default(),
            )?;
            transport::send(request, |chunk| {
                use std::io::Write;
                print!("{chunk}");
                let _ = std::io::stdout().flush();
            })?;
            println!();
        }
        _ => return Err("Unknown option; use --help.".into()),
    }
    Ok(())
}
fn save_new(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut f = options.open(path).map_err(|e| e.to_string())?;
    f.write_all(bytes).map_err(|e| e.to_string())
}
fn execute(id: &str, input: &str, second: &str) -> Result<String, String> {
    bellobox_core::validate_input(input)?;
    bellobox_core::validate_input(second)?;
    match id {
        "textTools"=>Ok(match second.trim(){"upper"=>text::case(input,text::CaseStyle::Upper),"lower"=>text::case(input,text::CaseStyle::Lower),"title"=>text::case(input,text::CaseStyle::Title),"sentence"=>text::case(input,text::CaseStyle::Sentence),"camel"=>text::case(input,text::CaseStyle::Camel),"pascal"=>text::case(input,text::CaseStyle::Pascal),"snake"=>text::case(input,text::CaseStyle::Snake),"kebab"=>text::case(input,text::CaseStyle::Kebab),"constant"=>text::case(input,text::CaseStyle::Constant),"base64"=>text::encode(input,text::Encoding::Base64),"url"=>text::encode(input,text::Encoding::Url),"html"=>text::encode(input,text::Encoding::Html),"hex"=>text::encode(input,text::Encoding::Hex),"decode"=>text::auto_decode(input)?.1,"decode-base64"=>text::decode(input,text::Encoding::Base64)?,"decode-url"=>text::decode(input,text::Encoding::Url)?,"decode-html"=>text::decode(input,text::Encoding::Html)?,"decode-hex"=>text::decode(input,text::Encoding::Hex)?,"pretty"=>{#[cfg(feature="developer-tools")] {bellobox_core::developer::execute("json",input,"pretty")?} #[cfg(not(feature="developer-tools"))] {return Err("Formatting is not included in this minimal build.".into());}},"hash"=>text::hashes(input),"sort"=>text::lines(input,text::LineOperation::Sort),"sort-reverse"=>text::lines(input,text::LineOperation::SortReverse),"nonempty"=>text::lines(input,text::LineOperation::Nonempty),"unique"=>text::lines(input,text::LineOperation::Unique),"trim"=>text::lines(input,text::LineOperation::Trim),"reverse"=>text::lines(input,text::LineOperation::Reverse),_=>{let c=text::counts(input,second);format!("{} characters (Unicode scalars)\n{} without whitespace\n{} words\n{} lines\n~{} tokens ({})\n\nOperation in second input: upper, lower, title, sentence, camel, pascal, snake, kebab, constant, base64, url, html, hex, decode, hash, sort, unique, trim, reverse.",c.characters,c.without_whitespace,c.words,c.lines,c.estimated_tokens,c.tokenizer_family)}}),
        "qr"=>bellobox_core::qr::terminal(input),
        "worldClock"=>{let mut p=Planner::default();if !input.trim().is_empty(){p.set_instant(bellobox_core::clock::parse_instant(input)?)?;}
if !second.trim().is_empty(){p.set_zones(&second.split(',').map(|s|s.trim().to_string()).collect::<Vec<_>>())?;}Ok(p.summary())},
        "home"=>Ok(format!("BelloBox Rust + GPUI\n\n{}\n\nNo selected text or clipboard history is persisted.\nRust settings: {}\n\nNative macOS capture, selection, recording, and updater parity remain in progress.",bello_platform::Platform::new().status().capabilities.iter().map(|c|format!("{:?}: {:?} — {}",c.capability,c.state,c.reason)).collect::<Vec<_>>().join("\n"),config_dir().display())),
        "settings"=>{let settings=Settings::load(&config_dir().join("settings.json"))?;serde_json::to_string_pretty(&settings).map_err(|e|e.to_string())},
        "ai"=>Err("AI sends only from the explicit Send AI action. Configure endpoint/model through BELLOBOX_AI_* environment variables; keys stay in memory.".into()),
        "screenshot"=>Ok("Use Capture full screen to save a new PNG, or OCR local image for offline text recognition. These actions require a working display server/helper or macOS permissions. Area/window selection and annotations are not yet ported.".into()),
        "recording"=>Err("The recording lifecycle host is available in the desktop app. Native screen recording remains unavailable; no capture or permissions were requested.".into()),
        "scrollCapture"|"videoToGIF"=>Err("This native workflow is not yet wired into the Rust UI. Full-screen capture and local OCR are available through the --capture/--ocr commands when the platform status reports them available. See rust/docs/bellobox-parity.md.".into()),
        _=>{
            #[cfg(feature="developer-tools")] {bellobox_core::developer::execute(id,input,second)}
            #[cfg(not(feature="developer-tools"))] {Err("Developer utility engines are excluded in this minimal build. Build with the default developer-tools feature to include them.".into())}
        },
    }
}
