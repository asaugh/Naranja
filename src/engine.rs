use std::{
    ffi::OsStr,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command,
};

const IMAGES: &[&str] = &[
    "jpg", "jpeg", "png", "webp", "heic", "tiff", "bmp", "gif", "avif",
];
const AUDIO: &[&str] = &["mp3", "m4a", "wav", "flac", "ogg", "opus", "aiff", "wma"];
const VIDEO: &[&str] = &["mp4", "mov", "mkv", "webm", "avi", "wmv", "gif"];

pub fn extension(path: &Path) -> String {
    path.extension()
        .and_then(OsStr::to_str)
        .unwrap_or_default()
        .to_ascii_lowercase()
}

pub fn kind(path: &Path) -> &'static str {
    let ext = extension(path);
    if path.is_dir() {
        "Folder"
    } else if IMAGES.contains(&ext.as_str()) {
        "Image"
    } else if AUDIO.contains(&ext.as_str()) {
        "Audio"
    } else if VIDEO.contains(&ext.as_str()) {
        "Video"
    } else if ["txt", "md", "rtf", "html", "doc", "docx", "odt"].contains(&ext.as_str()) {
        "Document"
    } else if ["csv", "tsv"].contains(&ext.as_str()) {
        "Spreadsheet"
    } else if ["srt", "vtt"].contains(&ext.as_str()) {
        "Subtitle"
    } else if ["zip", "tar", "gz"].contains(&ext.as_str()) {
        "Archive"
    } else {
        "File"
    }
}

pub fn formats(path: &Path) -> Vec<&'static str> {
    let ext = extension(path);
    if path.is_dir() {
        return vec!["zip", "tar"];
    }
    match kind(path) {
        "Image" => ["jpg", "png", "webp", "heic", "tiff", "bmp", "gif", "avif"]
            .into_iter()
            .filter(|f| *f != ext)
            .collect(),
        "Audio" => AUDIO.iter().copied().filter(|f| *f != ext).collect(),
        "Video" => ["mp4", "mov", "mkv", "webm", "avi", "wmv", "gif", "mp3"]
            .into_iter()
            .filter(|f| *f != ext)
            .collect(),
        "Document" => ["txt", "rtf", "html", "docx", "odt"]
            .into_iter()
            .filter(|f| *f != ext)
            .collect(),
        "Spreadsheet" => ["csv", "tsv"].into_iter().filter(|f| *f != ext).collect(),
        "Subtitle" => ["srt", "vtt", "txt"]
            .into_iter()
            .filter(|f| *f != ext)
            .collect(),
        "Archive" => Vec::new(),
        _ => Vec::new(),
    }
}

// Reserve a distinct output name before running a converter, so the original is never overwritten.
fn output_path(input: &Path, target: &str) -> io::Result<PathBuf> {
    let parent = input.parent().unwrap_or_else(|| Path::new("."));
    let stem = if input.is_dir() {
        input.file_name()
    } else {
        input.file_stem()
    }
    .and_then(OsStr::to_str)
    .unwrap_or("converted");
    for number in 0..10_000 {
        let suffix = if number == 0 {
            String::new()
        } else {
            format!(" {number}")
        };
        let candidate = parent.join(format!("{stem}{suffix}.{target}"));
        if !candidate.exists() && candidate != input {
            return Ok(candidate);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "No unused output name found",
    ))
}

fn run(program: &str, args: &[&OsStr]) -> Result<(), String> {
    let output = Command::new(binary(program))
        .args(args)
        .output()
        .map_err(|error| format!("Could not start {program}: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let detail = String::from_utf8_lossy(&output.stderr);
        Err(format!("{program} failed: {}", detail.trim()))
    }
}

// Finder launches apps with a smaller PATH than a terminal, so check Homebrew's
// standard locations as well as the inherited path before starting a tool.
pub fn binary(name: &str) -> PathBuf {
    let mut directories: Vec<PathBuf> =
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect();
    directories.extend([
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ]);
    directories
        .into_iter()
        .map(|dir| dir.join(name))
        .find(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from(name))
}

fn checked_run(program: &str, args: &[&OsStr], output: &Path) -> Result<(), String> {
    if let Err(error) = run(program, args) {
        let _ = fs::remove_file(output);
        return Err(error);
    }
    if output.exists() {
        Ok(())
    } else {
        Err(format!("{program} did not create an output file"))
    }
}

pub fn convert(input: &Path, target: &str) -> Result<PathBuf, String> {
    if !input.exists() {
        return Err("Input no longer exists".into());
    }
    if !formats(input).contains(&target) {
        return Err(format!("Unsupported conversion to {target}"));
    }
    let output = output_path(input, target).map_err(|e| e.to_string())?;
    let source = extension(input);
    let input_os = input.as_os_str();
    let output_os = output.as_os_str();

    match kind(input) {
        "Image" => {
            // macOS sips handles HEIC and common still formats; FFmpeg covers the rest.
            let sips_format = match target {
                "jpg" => "jpeg",
                "tiff" => "tiff",
                "png" => "png",
                "gif" => "gif",
                "bmp" => "bmp",
                "heic" => "heic",
                _ => "",
            };
            if !sips_format.is_empty() && source != "avif" && source != "webp" {
                let args = [
                    OsStr::new("-s"),
                    OsStr::new("format"),
                    OsStr::new(sips_format),
                    input_os,
                    OsStr::new("--out"),
                    output_os,
                ];
                if checked_run("sips", &args, &output).is_ok() {
                    return Ok(output);
                }
            }
            checked_run(
                "ffmpeg",
                &[
                    OsStr::new("-nostdin"),
                    OsStr::new("-i"),
                    input_os,
                    OsStr::new("-frames:v"),
                    OsStr::new("1"),
                    output_os,
                ],
                &output,
            )?;
        }
        "Audio" | "Video" => {
            let mut args = vec![OsStr::new("-nostdin"), OsStr::new("-i"), input_os];
            if target == "gif" {
                args.extend([
                    OsStr::new("-vf"),
                    OsStr::new("fps=12,scale=640:-1:flags=lanczos"),
                ]);
            }
            if target == "mp3" {
                args.extend([OsStr::new("-vn"), OsStr::new("-q:a"), OsStr::new("2")]);
            }
            args.push(output_os);
            checked_run("ffmpeg", &args, &output)?;
        }
        "Document" => {
            let format = match target {
                "txt" => "txt",
                "rtf" => "rtf",
                "html" => "html",
                "docx" => "docx",
                "odt" => "odt",
                _ => unreachable!(),
            };
            checked_run(
                "textutil",
                &[
                    OsStr::new("-convert"),
                    OsStr::new(format),
                    input_os,
                    OsStr::new("-output"),
                    output_os,
                ],
                &output,
            )?;
        }
        "Spreadsheet" => {
            let from = if source == "csv" { ',' } else { '\t' };
            let to = if target == "csv" { ',' } else { '\t' };
            let data = fs::read_to_string(input).map_err(|e| e.to_string())?;
            let rows = parse_delimited(&data, from)?;
            let mut file = fs::File::create(&output).map_err(|e| e.to_string())?;
            for row in rows {
                let line = row
                    .into_iter()
                    .map(|cell| quote_cell(&cell, to))
                    .collect::<Vec<_>>()
                    .join(&to.to_string());
                writeln!(file, "{line}").map_err(|e| e.to_string())?;
            }
        }
        "Subtitle" => {
            let content = fs::read_to_string(input).map_err(|e| e.to_string())?;
            let converted = convert_subtitles(&content, target);
            fs::write(&output, converted).map_err(|e| e.to_string())?;
        }
        "Folder" => {
            let parent = input.parent().unwrap_or_else(|| Path::new("."));
            let name = input.file_name().ok_or("Invalid archive path")?;
            if target == "zip" {
                checked_run(
                    "ditto",
                    &[
                        OsStr::new("-c"),
                        OsStr::new("-k"),
                        OsStr::new("--sequesterRsrc"),
                        OsStr::new("--keepParent"),
                        input_os,
                        output_os,
                    ],
                    &output,
                )?;
            } else {
                checked_run(
                    "tar",
                    &[
                        OsStr::new("-cf"),
                        output_os,
                        OsStr::new("-C"),
                        parent.as_os_str(),
                        name,
                    ],
                    &output,
                )?;
            }
        }
        _ => return Err("This file type has no conversion yet".into()),
    }
    Ok(output)
}

fn convert_subtitles(content: &str, target: &str) -> String {
    let lines: Vec<String> = content
        .lines()
        .filter(|line| *line != "WEBVTT")
        .map(|line| {
            if line.contains(" --> ") {
                match target {
                    "vtt" => line.replace(',', "."),
                    "srt" => line.replace('.', ","),
                    _ => String::new(),
                }
            } else if target == "txt" && line.trim().chars().all(|c| c.is_ascii_digit()) {
                String::new()
            } else {
                line.to_owned()
            }
        })
        .collect();
    if target == "vtt" {
        format!("WEBVTT\n\n{}", lines.join("\n").trim_start())
    } else {
        lines.join("\n").trim_start().to_owned()
    }
}

pub fn compress_zip(input: &Path) -> Result<PathBuf, String> {
    if !input.exists() {
        return Err("Input no longer exists".into());
    }
    let output = output_path(input, "zip").map_err(|e| e.to_string())?;
    checked_run(
        "ditto",
        &[
            OsStr::new("-c"),
            OsStr::new("-k"),
            OsStr::new("--sequesterRsrc"),
            OsStr::new("--keepParent"),
            input.as_os_str(),
            output.as_os_str(),
        ],
        &output,
    )?;
    Ok(output)
}

pub fn extract(input: &Path) -> Result<PathBuf, String> {
    let ext = extension(input);
    if !["zip", "tar", "gz"].contains(&ext.as_str()) {
        return Err("Not a supported archive".into());
    }
    let parent = input.parent().unwrap_or_else(|| Path::new("."));
    let stem = input
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or("extracted");
    // A .gz may contain one ordinary file rather than a TAR archive.
    if ext == "gz"
        && !Command::new(binary("tar"))
            .arg("-tf")
            .arg(input)
            .output()
            .map_err(|e| e.to_string())?
            .status
            .success()
    {
        let mut file = parent.join(stem);
        for n in 1..10_000 {
            if !file.exists() {
                break;
            }
            file = parent.join(format!("{stem} {n}"));
        }
        let output = fs::File::create(&file).map_err(|e| e.to_string())?;
        let status = Command::new(binary("gzip"))
            .arg("-dc")
            .arg(input)
            .stdout(output)
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            let _ = fs::remove_file(&file);
            return Err("Could not decompress GZIP file".into());
        }
        return Ok(file);
    }
    let mut folder = parent.join(stem);
    for n in 1..10_000 {
        if !folder.exists() {
            break;
        }
        folder = parent.join(format!("{stem} {n}"));
    }
    fs::create_dir(&folder).map_err(|e| e.to_string())?;
    let result = if ext == "zip" {
        run(
            "ditto",
            &[
                OsStr::new("-x"),
                OsStr::new("-k"),
                input.as_os_str(),
                folder.as_os_str(),
            ],
        )
    } else {
        run(
            "tar",
            &[
                OsStr::new("-xf"),
                input.as_os_str(),
                OsStr::new("-C"),
                folder.as_os_str(),
            ],
        )
    };
    if let Err(error) = result {
        let _ = fs::remove_dir_all(&folder);
        return Err(error);
    }
    Ok(folder)
}

// CSV and TSV need quote-aware parsing: simply replacing separators corrupts quoted cells.
fn parse_delimited(input: &str, separator: char) -> Result<Vec<Vec<String>>, String> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' if quoted && chars.peek() == Some(&'"') => {
                cell.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            c if c == separator && !quoted => {
                row.push(std::mem::take(&mut cell));
            }
            '\n' if !quoted => {
                row.push(std::mem::take(&mut cell));
                rows.push(std::mem::take(&mut row));
            }
            '\r' if !quoted && chars.peek() == Some(&'\n') => {}
            c => cell.push(c),
        }
    }
    if quoted {
        return Err("Unclosed quoted field".into());
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    Ok(rows)
}

fn quote_cell(cell: &str, separator: char) -> String {
    if cell.contains(separator) || cell.contains('"') || cell.contains('\n') {
        format!("\"{}\"", cell.replace('"', "\"\""))
    } else {
        cell.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoted_csv_survives_tsv_round_trip() {
        let rows = parse_delimited("name,note\nAda,\"hello, \"\"world\"\"\"\n", ',').unwrap();
        assert_eq!(rows[1][1], "hello, \"world\"");
        let tsv = rows[1]
            .iter()
            .map(|cell| quote_cell(cell, '\t'))
            .collect::<Vec<_>>()
            .join("\t");
        assert_eq!(
            parse_delimited(&tsv, '\t').unwrap()[0][1],
            "hello, \"world\""
        );
    }

    #[test]
    fn output_names_do_not_replace_inputs() {
        let path = Path::new("/tmp/photo.png");
        assert_ne!(output_path(path, "png").unwrap(), path);
    }

    #[test]
    fn subtitle_text_keeps_punctuation() {
        let input = "WEBVTT\n\n00:00:01.000 --> 00:00:03.000\nDr. Orange says hi.\n";
        let srt = convert_subtitles(input, "srt");
        assert!(srt.contains("00:00:01,000 --> 00:00:03,000"));
        assert!(srt.contains("Dr. Orange says hi."));
    }
}
