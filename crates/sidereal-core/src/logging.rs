use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct RunLogFile {
    pub file: File,
    pub path: PathBuf,
}

pub fn workspace_logs_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("logs")
}

pub fn prepare_timestamped_log_file(service_name: &str) -> io::Result<RunLogFile> {
    prepare_timestamped_log_file_in_dir(service_name, &workspace_logs_dir())
}

pub fn prepare_timestamped_log_file_in_dir(
    service_name: &str,
    logs_dir: &Path,
) -> io::Result<RunLogFile> {
    fs::create_dir_all(logs_dir)?;
    let timestamp = unix_timestamp_for_filename(SystemTime::now())?;
    let pid = std::process::id();

    for attempt in 0..1000 {
        let suffix = if attempt == 0 {
            String::new()
        } else {
            format!("-{attempt}")
        };
        let path = logs_dir.join(format!("{service_name}-{timestamp}-pid{pid}{suffix}.log"));
        match OpenOptions::new().create_new(true).append(true).open(&path) {
            Ok(file) => return Ok(RunLogFile { file, path }),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        }
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!(
            "failed to allocate unique log file for {service_name} in {}",
            logs_dir.display()
        ),
    ))
}

fn unix_timestamp_for_filename(now: SystemTime) -> io::Result<String> {
    let elapsed = now.duration_since(UNIX_EPOCH).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("system clock is before the Unix epoch: {err}"),
        )
    })?;
    Ok(format!(
        "unix-{}-{:03}",
        elapsed.as_secs(),
        elapsed.subsec_millis()
    ))
}

#[cfg(test)]
mod tests {
    use super::prepare_timestamped_log_file_in_dir;
    use std::fs;

    #[test]
    fn prepare_timestamped_log_file_in_dir_creates_unique_files() {
        let temp_root =
            std::env::temp_dir().join(format!("sidereal-log-test-{}", uuid::Uuid::new_v4()));

        let first = prepare_timestamped_log_file_in_dir("sidereal-gateway", &temp_root).unwrap();
        let second = prepare_timestamped_log_file_in_dir("sidereal-gateway", &temp_root).unwrap();

        assert!(first.path.starts_with(&temp_root));
        assert!(second.path.starts_with(&temp_root));
        assert!(first.path.exists());
        assert!(second.path.exists());
        assert_ne!(first.path, second.path);

        let first_name = first.path.file_name().unwrap().to_string_lossy();
        let second_name = second.path.file_name().unwrap().to_string_lossy();
        assert!(first_name.starts_with("sidereal-gateway-unix-"));
        assert!(second_name.starts_with("sidereal-gateway-unix-"));
        assert!(first_name.ends_with(".log"));
        assert!(second_name.ends_with(".log"));

        drop(first);
        drop(second);
        fs::remove_dir_all(temp_root).unwrap();
    }
}
