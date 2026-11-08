//! `parcel` command-line interface. Uses exactly the same engine (parcel-core) as the desktop app.
use parcel_core::cancel::{CancelToken, Progress};
use parcel_core::compare::compare_manifests;
use parcel_core::limits::ResourceLimits;
use parcel_core::manifest::{parse_manifest, Entry, EntryKind, Manifest};
use parcel_core::reader::{ConflictPolicy, ExtractOptions, PackageReader};
use parcel_core::writer::{ExportOptions, PackageWriter};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Writes a line to stdout, silently ignoring a closed pipe (e.g. `parcel inspect x | head`).
macro_rules! out {
    ($($a:tt)*) => {{
        use std::io::Write;
        let _ = writeln!(std::io::stdout(), $($a)*);
    }};
}

const HELP: &str = "parcel — inspect, verify, extract and create .parcel packages

USAGE:
  parcel inspect  <package> [--json]
  parcel verify   <package> [--json]
  parcel extract  <package> <dest-dir> [--select <path>]... [--on-conflict fail|skip|rename]
  parcel create   <out.parcel> <file-or-dir>... [--name <name>] [--overwrite] [--no-compress]
  parcel validate <manifest.json | package>
  parcel compare  <old-package> <new-package> [--json]
  parcel import-zip <in.zip> <out.parcel> [--name <name>] [--overwrite]

Exit status: 0 success, 1 failure (e.g. verification failed), 2 usage error.";

struct Args {
    pos: Vec<String>,
    json: bool,
    overwrite: bool,
    no_compress: bool,
    name: Option<String>,
    select: Vec<String>,
    conflict: ConflictPolicy,
}

fn parse(raw: &[String]) -> Result<Args, String> {
    let mut a =
        Args { pos: vec![], json: false, overwrite: false, no_compress: false, name: None, select: vec![], conflict: ConflictPolicy::Fail };
    let mut it = raw.iter();
    while let Some(arg) = it.next() {
        let mut value = |what: &str| -> Result<String, String> { it.next().cloned().ok_or_else(|| format!("{what} needs a value")) };
        match arg.as_str() {
            "--json" => a.json = true,
            "--overwrite" => a.overwrite = true,
            "--no-compress" => a.no_compress = true,
            "--name" => a.name = Some(value("--name")?),
            "--select" => a.select.push(value("--select")?),
            "--on-conflict" => {
                a.conflict = match value("--on-conflict")?.as_str() {
                    "fail" => ConflictPolicy::Fail,
                    "skip" => ConflictPolicy::Skip,
                    "rename" => ConflictPolicy::Rename,
                    o => return Err(format!("unknown conflict policy {o:?}")),
                }
            }
            s if s.starts_with("--") => return Err(format!("unknown option {s}")),
            s => a.pos.push(s.to_string()),
        }
    }
    Ok(a)
}

fn human(n: u64) -> String {
    let u = ["B", "KB", "MB", "GB", "TB"];
    let (mut v, mut i) = (n as f64, 0);
    while v >= 1024.0 && i < u.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{n} B")
    } else {
        format!("{v:.1} {}", u[i])
    }
}

fn progress_printer() -> impl Fn(&Progress) + Send + Sync {
    |p: &Progress| {
        if std::io::IsTerminal::is_terminal(&std::io::stderr()) && !p.current_item.is_empty() {
            eprint!("\r{:<10} {}/{}  {:<50.50}", p.phase, p.items_done, p.items_total, p.current_item);
        }
    }
}

fn run(raw: Vec<String>) -> Result<u8, String> {
    let Some(cmd) = raw.first().cloned() else { return Err("missing command".into()) };
    if matches!(cmd.as_str(), "-h" | "--help" | "help") {
        out!("{HELP}");
        return Ok(0);
    }
    let a = parse(&raw[1..])?;
    let limits = ResourceLimits::default();
    let cancel = CancelToken::new();
    let pp = progress_printer();
    let e2s = |e: parcel_core::ParcelError| format!("{} ({})", e, e.code());
    match cmd.as_str() {
        "inspect" => {
            let [pkg] = a.pos.as_slice() else { return Err("inspect needs <package>".into()) };
            let mut r = PackageReader::open(Path::new(pkg), &limits).map_err(e2s)?;
            let m = r.manifest().clone();
            if a.json {
                out!("{}", serde_json::to_string_pretty(&m).map_err(|e| e.to_string())?);
            } else {
                out!("Name:        {}\nDescription: {}\nPackage ID:  {}\nRevision:    {}\nFormat:      {} {}\nCreated:     {}\nModified:    {}\nGenerator:   {} {}\nEntries:     {}  ({} stored)\nReferences:  {}\nDependencies:{}\nFile size:   {}",
                    m.name, m.description, m.package_id, m.revision, m.format, m.format_version, m.created_at, m.modified_at,
                    m.generator.name, m.generator.version, m.entries.len(), human(m.total_content_size()), m.references.len(), m.dependencies.len(), human(r.file_size()));
                out!("\nContents:");
                for e in &m.entries {
                    let k = match e.kind {
                        EntryKind::File => "file  ",
                        EntryKind::Folder => "folder",
                        EntryKind::Note => "note  ",
                        EntryKind::Link => "link  ",
                    };
                    out!(
                        "  {k} {:>10}  {}",
                        if e.kind == EntryKind::Folder || e.kind == EntryKind::Link { String::new() } else { human(e.size) },
                        e.path
                    );
                }
                let notes = r.notes().unwrap_or_default();
                if !notes.trim().is_empty() {
                    out!("\nWorkspace notes:\n{notes}");
                }
            }
            Ok(0)
        }
        "verify" => {
            let [pkg] = a.pos.as_slice() else { return Err("verify needs <package>".into()) };
            let r = PackageReader::open(Path::new(pkg), &limits).map_err(e2s)?;
            let rep = r.verify(&cancel, &pp).map_err(e2s)?;
            eprintln!();
            if a.json {
                out!("{}", serde_json::to_string_pretty(&rep).map_err(|e| e.to_string())?);
            } else {
                print!("{}", rep.to_text());
            }
            Ok(if rep.ok { 0 } else { 1 })
        }
        "extract" => {
            let [pkg, dest] = a.pos.as_slice() else { return Err("extract needs <package> <dest-dir>".into()) };
            let mut r = PackageReader::open(Path::new(pkg), &limits).map_err(e2s)?;
            let selection = if a.select.is_empty() {
                None
            } else {
                let mut ids = vec![];
                for s in &a.select {
                    let e = r
                        .manifest()
                        .entries
                        .iter()
                        .find(|e| e.path.eq_ignore_ascii_case(s.trim_end_matches('/')))
                        .ok_or_else(|| format!("no entry at path {s:?}"))?;
                    ids.push(e.id.clone());
                }
                Some(ids)
            };
            let rep = r.extract(Path::new(dest), &ExtractOptions { selection, conflict: a.conflict, limits }, &cancel, &pp).map_err(e2s)?;
            eprintln!();
            out!("Extracted {} file(s), {} skipped, {} written.", rep.written.len(), rep.skipped.len(), human(rep.bytes_written));
            Ok(0)
        }
        "create" => {
            let (out, inputs) = match a.pos.split_first() {
                Some((o, rest)) if !rest.is_empty() => (o.clone(), rest.to_vec()),
                _ => return Err("create needs <out.parcel> and at least one input".into()),
            };
            let name =
                a.name.clone().unwrap_or_else(|| Path::new(&out).file_stem().and_then(|s| s.to_str()).unwrap_or("Package").to_string());
            let mut files: Vec<(String, PathBuf)> = vec![];
            let mut folders: Vec<String> = vec![];
            for i in &inputs {
                collect(Path::new(i), "", &mut files, &mut folders)?;
            }
            let total: u64 = files.iter().filter_map(|(_, p)| fs::metadata(p).ok()).map(|m| m.len()).sum();
            let opts = ExportOptions { overwrite: a.overwrite, compress: !a.no_compress, ..Default::default() };
            let mut w = PackageWriter::create(
                Path::new(&out),
                Manifest::new(name, ""),
                opts,
                (files.len() + folders.len()) as u64,
                total,
                &cancel,
                &pp,
            )
            .map_err(e2s)?;
            for f in folders {
                w.add_virtual(Entry::new(EntryKind::Folder, uuid::Uuid::new_v4().to_string(), f)).map_err(e2s)?;
            }
            for (logical, p) in files {
                let meta = fs::metadata(&p).map_err(|e| e.to_string())?;
                let mut e = Entry::new(EntryKind::File, uuid::Uuid::new_v4().to_string(), logical);
                e.modified_at = meta.modified().ok().map(Into::into);
                let mut f = fs::File::open(&p).map_err(|e| format!("{}: {e}", p.display()))?;
                w.add_stream(e, &mut f, Some(meta.len())).map_err(e2s)?;
            }
            let rep = w.finish("", None).map_err(e2s)?;
            eprintln!();
            out!("Created {} ({} entries, {}).", rep.path.display(), rep.entry_count, human(rep.package_size));
            Ok(0)
        }
        "validate" => {
            let [target] = a.pos.as_slice() else { return Err("validate needs <manifest.json | package>".into()) };
            let res = if fs::metadata(target).map(|m| m.is_file()).unwrap_or(false) && target.ends_with(".json") {
                parse_manifest(&fs::read(target).map_err(|e| e.to_string())?, &limits).map(|_| ())
            } else {
                PackageReader::open(Path::new(target), &limits).map(|_| ())
            };
            match res {
                Ok(()) => {
                    out!("OK: manifest is valid.");
                    Ok(0)
                }
                Err(e) => {
                    out!("INVALID: {}", e2s(e));
                    Ok(1)
                }
            }
        }
        "compare" => {
            let [old, new] = a.pos.as_slice() else { return Err("compare needs <old-package> <new-package>".into()) };
            let o = PackageReader::open(Path::new(old), &limits).map_err(e2s)?;
            let n = PackageReader::open(Path::new(new), &limits).map_err(e2s)?;
            let c = compare_manifests(o.manifest(), n.manifest());
            if a.json {
                out!("{}", serde_json::to_string_pretty(&c).map_err(|e| e.to_string())?);
            } else {
                out!("Compared by checksum and metadata only (no semantic content diff).");
                for (label, v) in [
                    ("Added", &c.added),
                    ("Removed", &c.removed),
                    ("Changed", &c.changed),
                    ("Moved", &c.moved),
                    ("Metadata changed", &c.metadata_changed),
                ] {
                    for d in v {
                        out!("{label:<17} {}  {}", d.path, d.detail.join("; "));
                    }
                }
                for d in &c.package_differences {
                    out!("Package           {d}");
                }
                out!("Unchanged: {}", c.unchanged.len());
            }
            Ok(if c.is_identical() { 0 } else { 1 })
        }
        "import-zip" => {
            let [zip, out] = a.pos.as_slice() else { return Err("import-zip needs <in.zip> <out.parcel>".into()) };
            let name =
                a.name.clone().unwrap_or_else(|| Path::new(zip).file_stem().and_then(|s| s.to_str()).unwrap_or("Imported").to_string());
            let opts = ExportOptions { overwrite: a.overwrite, ..Default::default() };
            let rep =
                parcel_core::zipimport::convert_zip_to_parcel(Path::new(zip), Path::new(out), &name, opts, &cancel, &pp).map_err(e2s)?;
            eprintln!();
            for w in &rep.warnings {
                eprintln!("warning: {w}");
            }
            out!("Created {} ({} entries, {}).", rep.build.path.display(), rep.build.entry_count, human(rep.build.package_size));
            Ok(0)
        }
        other => Err(format!("unknown command {other:?}")),
    }
}

fn collect(p: &Path, prefix: &str, files: &mut Vec<(String, PathBuf)>, folders: &mut Vec<String>) -> Result<(), String> {
    let meta = fs::symlink_metadata(p).map_err(|e| format!("{}: {e}", p.display()))?;
    if meta.file_type().is_symlink() {
        eprintln!("warning: skipping symbolic link {}", p.display());
        return Ok(());
    }
    let name = parcel_core::pathsafe::sanitize_component(
        &p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "untitled".into()),
    );
    let logical = if prefix.is_empty() { name } else { format!("{prefix}/{name}") };
    if meta.is_dir() {
        folders.push(logical.clone());
        let mut kids: Vec<_> = fs::read_dir(p).map_err(|e| e.to_string())?.filter_map(|e| e.ok().map(|e| e.path())).collect();
        kids.sort();
        for k in kids {
            collect(&k, &logical, files, folders)?;
        }
    } else if meta.is_file() {
        files.push((logical, p.to_path_buf()));
    }
    Ok(())
}

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    match run(raw) {
        Ok(c) => ExitCode::from(c),
        Err(e) => {
            eprintln!("error: {e}\n\n{HELP}");
            ExitCode::from(if e.contains(" (") && e.ends_with(')') { 1 } else { 2 })
        }
    }
}
