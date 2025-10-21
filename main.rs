use std::{
    fs::{read, File},
    io::Write,
    path::PathBuf,
    time::Duration,
};

use clap::Parser;

mod api;
mod daemon;
mod utils;

use api::*;
use daemon::*;
use utils::DurationWrapper;

#[derive(Parser)]
struct Args {
    #[arg(short, long, default_value_t = false)]
    daemonize: bool,

    #[arg(short, long, default_value = None)]
    api_key: Option<String>,

    #[arg(long)]
    working_directory: Option<String>,

    #[arg(short, long, default_value_t = false)]
    terminate: bool,

    #[arg(short, long, default_value_t = false)]
    save_to_config: bool,

    #[arg(long, default_value_t = true)]
    current_weather: bool,

    #[arg(long, default_value_t = true)]
    forecast: bool,

    #[arg(long, default_value_t = false)]
    alerts: bool,

    #[arg(long, default_value = None)]
    city: Option<String>,

    #[arg(long, default_value_t = Duration::new(600,0).into())]
    daemon_update_interval: DurationWrapper,

    #[arg(long, default_value_t = Duration::new(21600,0).into())]
    daemon_notif_interval: DurationWrapper,
}

fn main() {
    let args = Args::parse();
    let working_directory = args
        .working_directory
        .as_ref()
        .map(|s| s.into())
        .unwrap_or(default_working_directory());

    let api_key = args.api_key.clone().or_else(|| load_api_key(&working_directory)).expect(
        "TODO: implement config file, failed to grab api_key. An api key can be specified with the -a option",
    );
    let mut api = Api::new(api_key.clone());

    let mut api_config = ApiRequestConfiguration::default();
    let mut daemon_config = DaemonConfiguration::default();

    load_from_config(&working_directory, &mut daemon_config, &mut api_config);

    daemon_config.exec_interval = args.daemon_update_interval.clone().into();
    daemon_config.notif_interval = args.daemon_notif_interval.clone().into();

    api_config.requests = api::RequestTypes {
        current: args.current_weather,
        alerts: args.alerts,
        forecast: args.forecast,
    };

    if let Some(city) = args.city {
        api_config.q = Location::City(city);
    }

    if let Some(path) = args.working_directory {
        daemon_config.working_directory = path.into();
    }

    if let Some(days) = Some(3) {
        api_config.days = Some(days);
    }

    if args.save_to_config {
        save_to_config(
            &working_directory,
            daemon_config.clone(),
            api_config.clone(),
            api_key,
        );
    }

    let response = api.make_request(&api_config);
    if let Some(Err(e)) = response.current {
        println!("Get Current Weather Api Call failed with: {e:?}");
    };
    if let Some(Err(e)) = response.alerts {
        println!("Get Weather Alerts Api Call failed with: {e:?}");
    }
    if let Some(Err(e)) = response.forecast {
        println!("Get Forecast Api Call failed with: {e:?}");
    }

    if let Some((response, _timestamp)) = api.get_cached_current() {
        println!("{:=^32}", "Current Weather");
        println!(
            "Current Tempurature (Imperial): {}°F, Feels like: {}°F",
            response["current"]["temp_f"], response["current"]["feelslike_f"]
        );
        println!(
            "Current Tempurature (Metric): {}°C, Feels like: {}°C",
            response["current"]["temp_c"], response["current"]["feelslike_c"]
        );
        println!(
            "Wind Speed (Imperial): {} mph, from {}",
            response["current"]["wind_mph"], response["current"]["wind_dir"]
        );
        println!(
            "Wind Speed (Metric): {} kph, from {}",
            response["current"]["wind_kph"], response["current"]["wind_dir"]
        );
        println!(
            "Wind Chill (Imperial): {}°F",
            response["current"]["windchill_f"]
        );
        println!(
            "Wind Chill (Metric): {}°C",
            response["current"]["windchill_c"]
        );
        println!("Humidity: {}%", response["current"]["humidity"]);
        println!(
            "Pressure (Imperial): {}in",
            response["current"]["pressure_in"]
        );
        println!(
            "Pressure (Metric): {}mb",
            response["current"]["pressure_mb"]
        );
        println!("Condition: {}\n", response["current"]["condition"]["text"]);
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

            println!("{}", alert["headline"]);
        }
    }

    if let Some((response, _timestamp)) = api.get_cached_forecast() {
        println!("{:=^32}", "Forecast");
        for i in 0..api_config.days.unwrap() {
            println!(
                "Weather in {} day(s)", i + 1
            );
            println!(
                "Tempurature Average: {}°F, {}°C",
                response["forecast"]["forecastday"][i]["day"]["avgtemp_f"],
                response["forecast"]["forecastday"][i]["day"]["avgtemp_c"]
            );
            println!(
                "Tempurature High: {}°F, {}°C",
                response["forecast"]["forecastday"][i]["day"]["maxtemp_f"],
                response["forecast"]["forecastday"][i]["day"]["maxtemp_c"]
            );
            println!(
                "Tempurature Low: {}°F, {}°C",
                response["forecast"]["forecastday"][i]["day"]["mintemp_f"],
                response["forecast"]["forecastday"][i]["day"]["mintemp_c"]
            );
            println!(
                "Max Wind Speed: {} mph, {} kph",
                response["forecast"]["forecastday"][i]["day"]["maxwind_mph"],
                response["forecast"]["forecastday"][i]["day"]["maxwind_kph"]
            );
            println!(
                "Average Humidity: {}%", 
                response["forecast"]["forecastday"][i]["day"]["avghumidity"]
            );
            println!(
                "Chance of Rain: {}%",
                response["forecast"]["forecastday"][i]["day"]["daily_chance_of_rain"]
            );
            println!(
                "Chance of Snow: {}%",
                response["forecast"]["forecastday"][i]["day"]["daily_chance_of_snow"]
            );
            println!(
                "Total Precipitation: {} in, {} mm",
                response["forecast"]["forecastday"][i]["day"]["totalprecip_in"],
                response["forecast"]["forecastday"][i]["day"]["totalprecip_mm"]
            );
            println!(
                "Condition: {}\n", 
                response["forecast"]["forecastday"][i]["day"]["condition"]["text"]
            );
        }
    }

    if args.terminate {
        if let Err(e) = terminate(working_directory.join("pid")) {
            eprintln!("Failed to terminate existing instance: {:?}", e);
        }
    }

    if args.daemonize {
        let config = DaemonConfiguration {
            working_directory,
            exec_interval: args.daemon_update_interval.into(),
            notif_interval: args.daemon_notif_interval.into(),
        };

        match daemon::daemonize(&config) {
            Ok(()) => println!("Process Spawned Successfully"),
            Err(DaemonizationError::UnsupportedOS) => {
                eprintln!("Daemonization option is not supported on your operating system")
            }
            Err(DaemonizationError::FailureToCreateLogFile) => {
                eprintln!(
                    "Failed to create log files in {}",
                    config.working_directory.to_str().unwrap()
                );
            }
            Err(DaemonizationError::FailureToCreateWorkingDirectory) => {
                eprintln!(
                    "Failed to create/open working directory {}",
                    config.working_directory.to_str().unwrap()
                );
            }
            Err(DaemonizationError::ExistingInstance) => {
                eprintln!("Can't start daemon when one already exists")
            }
        }
        daemon_main(api, daemon_config, api_config)
    }
}

fn save_to_config(
    working_directory: &PathBuf,
    daemon_config: DaemonConfiguration,
    api_config: ApiRequestConfiguration,
    api_key: String,
) {
    let Ok(mut file) = File::create(working_directory.join("api_key.ron")) else {
        eprintln!(
            "Failed to create file api_key.ron in {}",
            working_directory.to_str().unwrap()
        );
        return;
    };
    if let Err(_e) = write!(&mut file, "{}", api_key) {
        eprintln!(
            "Failed to write to api_key.ron in {}",
            working_directory.to_str().unwrap()
        );
    };
    let Ok(mut file) = File::create(working_directory.join("api_config.ron")) else {
        eprintln!(
            "Failed to create file api_config.ron in {}",
            working_directory.to_str().unwrap()
        );
        return;
    };
    if let Err(_e) = write!(&mut file, "{}", ron::to_string(&api_config).unwrap()) {
        eprintln!(
            "Failed to write to api_config.ron in {}",
            working_directory.to_str().unwrap()
        );
    };

    let Ok(mut file) = File::create(working_directory.join("daemon_config.ron")) else {
        eprintln!(
            "Failed to create file daemon_config.ron in {}",
            working_directory.to_str().unwrap()
        );
        return;
    };
    if let Err(_e) = write!(&mut file, "{}", ron::to_string(&daemon_config).unwrap()) {
        eprintln!(
            "Failed to write to daemon_config.ron in {}",
            working_directory.to_str().unwrap()
        );
    };
}

fn load_api_key(working_directory: &PathBuf) -> Option<String> {
    let api_key_path = working_directory.join("api_key.ron");
    match std::fs::read_to_string(&api_key_path) {
        Ok(key) => Some(key),
        Err(e) => {
            eprintln!(
                "Failed to read API key from {}: {:?}",
                api_key_path.to_str().unwrap(),
                e
            );
            None
        }
    }
}

fn load_from_config(
    working_directory: &PathBuf,
    daemon_config: &mut DaemonConfiguration,
    api_config: &mut ApiRequestConfiguration,
) {
    if let Ok(data) = read(&working_directory.join("api_config.ron")) {
        match ron::from_str::<ApiRequestConfiguration>(&String::from_utf8_lossy(&data)) {
            Ok(config) => *api_config = config,
            Err(e) => eprintln!("Failed to parse api_config.ron: {:?}", e),
        }
    } else {
        eprintln!(
            "Failed to open file api_config.ron in {}",
            working_directory.to_str().unwrap()
        );
    }
    // Load Daemon configuration
    if let Ok(data) = read(&working_directory.join("daemon_config.ron")) {
        match ron::from_str::<DaemonConfiguration>(&String::from_utf8_lossy(&data)) {
            Ok(config) => *daemon_config = config,
            Err(e) => eprintln!("Failed to parse daemon_config.ron: {:?}", e),
        }
    } else {
        eprintln!(
            "Failed to open file daemon_config.ron in {}",
            working_directory.to_str().unwrap()
        );
    }
}
