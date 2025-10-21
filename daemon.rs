use crate::api::*;
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    path::PathBuf,
    thread::sleep,
    time::{Duration, Instant},
};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct DaemonConfiguration {
    pub working_directory: PathBuf,
    pub exec_interval: Duration,
    pub notif_interval: Duration,
}

#[derive(Debug, Clone, Copy)]
pub enum DaemonizationError {
    UnsupportedOS,
    FailureToCreateLogFile,
    FailureToCreateWorkingDirectory,
    ExistingInstance,
}

#[allow(unused_attributes)]
pub fn daemonize(config: &DaemonConfiguration) -> Result<(), DaemonizationError> {
    #[cfg(unix)]
    return daemonize_unix(config);

    #[cfg(windows)]
    return daemonize_windows(config);

    #[allow(unreachable_code)]
    Err(DaemonizationError::UnsupportedOS)
}

#[cfg(unix)]
fn daemonize_unix(config: &DaemonConfiguration) -> Result<(), DaemonizationError> {
    use daemonize::Daemonize;
    use std::fs::File;

    let DaemonConfiguration {
        working_directory, ..
    } = config;

    let path = PathBuf::from(working_directory);

    if let Err(_) = File::open(path.clone()) {
        if let Err(_) = std::fs::create_dir(path.clone()) {
            return Err(DaemonizationError::FailureToCreateWorkingDirectory);
        };
    }

    let log = path.join("log");
    let log_err = path.join("log_err");
    let pid = path.join("pid");

    let Ok(log_file) = File::create(&log) else {
        return Err(DaemonizationError::FailureToCreateLogFile);
    };
    let Ok(log_err_file) = File::create(&log_err) else {
        return Err(DaemonizationError::FailureToCreateLogFile);
    };
    if let Ok(_) = File::open(&pid) {
        if let Err(e) = terminate(pid.clone()) {
            eprintln!("Failed to terminate existing instance: {:?}", e);
            return Err(DaemonizationError::ExistingInstance);
        }
    }

    Daemonize::new()
        .working_directory(path)
        .stdout(log_file)
        .stderr(log_err_file)
        .pid_file(pid)
        .start()
        .expect("Failed to start daemon");

    Ok(())
}

#[cfg(windows)]
fn daemonize_windows(config: &DaemonConfiguration) -> Result<(), DaemonizationError> {
    //Daemonize windows not functional yet, just run Daemon main
    Ok(())
}

#[derive(Copy, Clone, Debug)]
#[allow(dead_code)]
pub enum TerminationError {
    PidFileInaccessible,
    FailedToRemoveOldPid,
    ProcessKillFailed,
    UnsupportedOS,
}

pub fn terminate(pid_path: PathBuf) -> Result<(), TerminationError> {
    let Ok(pid) = std::fs::read_to_string(&pid_path) else {
        return Err(TerminationError::PidFileInaccessible);
    };
    let pid = pid.trim();

    if cfg!(unix) {
        if let Err(e) = std::process::Command::new("kill").args([pid]).output() {
            eprintln!("Failed to terminate process: {e:?}");
            return Err(TerminationError::ProcessKillFailed);
        }
    } else if cfg!(windows) {
        //daemonize not supported for windows
        return Err(TerminationError::ProcessKillFailed);
    } else {
        return Err(TerminationError::UnsupportedOS);
    }

    // if let Err(_) = std::fs::remove_file(pid_path) {
    //     return Err(TerminationError::FailedToRemoveOldPid);
    // }

    Ok(())
}

pub fn default_working_directory() -> PathBuf {
    if cfg!(unix) {
        home::home_dir().unwrap().join(".weathd")
    } else if cfg!(windows) {
        //FIX THIS, Change where the file is placed
        home::home_dir().unwrap().join(".weathd")
    } else {
        panic!("No default working directory for current operating system. Path must be passed with the --working-directory={{path}} option")
    }
}

pub fn daemon_main(
    mut api: Api,
    daemon_config: DaemonConfiguration,
    api_config: ApiRequestConfiguration,
) {
    use notify_rust::Notification;
    let start = Instant::now();
    let mut last_iteration = start.clone();
    let mut last_notif = last_iteration - daemon_config.notif_interval;

    loop {
        let response = api.make_request(&api_config);

        if let Some(Err(e)) = response.current {
            println!("{}", chrono::Local::now());
            println!("Get Current Weather Api Call failed with: {e:?}");
        }

        if let Some(Err(e)) = response.alerts {
            println!("{}", chrono::Local::now());
            println!("Get Weather Alerts Api Call failed with: {e:?}");
        }

        if let Some(Err(e)) = response.forecast {
            println!("{}", chrono::Local::now());
            println!("Get Forecast Api Call failed with: {e:?}");
        }

        if last_notif.elapsed() >= daemon_config.notif_interval {
            last_notif = Instant::now();
            if let Some((response, _timestamp)) = api.get_cached_current() {
                let mut notification = Vec::new();

                writeln!(
                    notification,
                    "Current Tempurature (Imperial): {}°F, Feels like: {}°F",
                    response["current"]["temp_f"], response["current"]["feelslike_f"]
                )
                    .unwrap();

                writeln!(
                    notification,
                    "Current Tempurature (Metric): {}°C, Feels like: {}°C",
                    response["current"]["temp_c"], response["current"]["feelslike_c"]
                )
                    .unwrap();

                writeln!(
                    notification,
                    "Wind Speed (Imperial): {} mph, from {}",
                    response["current"]["wind_mph"], response["current"]["wind_dir"]
                )
                    .unwrap();

                writeln!(
                    notification,
                    "Wind Speed (Metric): {} kph, from {}",
                    response["current"]["wind_kph"], response["current"]["wind_dir"]
                )
                    .unwrap();

                writeln!(
                    notification,
                    "Wind Chill (Imperial): {}°F",
                    response["current"]["windchill_f"]
                )
                    .unwrap();

                writeln!(
                    notification,
                    "Wind Chill (Metric): {}°C",
                    response["current"]["windchill_c"]
                )
                    .unwrap();

                writeln!(
                    notification,
                    "Humidity: {}%",
                    response["current"]["humidity"]
                )
                    .unwrap();

                writeln!(
                    notification,
                    "Pressure (Imperial): {}in",
                    response["current"]["pressure_in"]
                )
                    .unwrap();

                writeln!(
                    notification,
                    "Pressure (Metric): {}mb",
                    response["current"]["pressure_mb"]
                )
                    .unwrap();

                writeln!(
                    notification,
                    "Condition: {}",
                    response["current"]["condition"]["text"]
                )
                    .unwrap();

                let notification = String::from_utf8(notification)
                    .expect("Failed to format current weather notification");

                if let Err(e) = Notification::new()
                    .summary("Current Weather")
                        .body(&notification)
                        .show()
                {
                    println!("{}", chrono::Local::now());
                    println!("Failed to send current weather notification: {e:?}");
                };
            }

            if let Some((response, _timestamp)) = api.get_cached_alerts() {
                let alert_array = &response["alerts"]["alert"];
                if !alert_array.is_array() {
                    panic!("Received Invalid API alert response");
                }
                for alert in (0..).map(|i| &alert_array[i]) {
                    if alert.is_null() {
                        break;
                    }

                    if let Err(e) = Notification::new()
                        .summary(format!("{}", alert["headline"]).as_ref())
                            .body(format!("{}", alert["instruction"]).as_ref())
                            .show()
                    {
                        println!("{}", chrono::Local::now());
                        println!("Failed to send weather alerts notification: {e:?}");
                    };
                }
            }

            let next_iteration = last_iteration + daemon_config.exec_interval;
            last_iteration = Instant::now();

            if next_iteration > last_iteration {
                sleep(next_iteration - last_iteration);
            }
        }
    }
}
