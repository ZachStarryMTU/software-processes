use json::JsonValue;
use serde::{Deserialize, Serialize};
use std::{fmt::Display, time::Instant};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ApiRequestConfiguration {
    pub q: Location,
    pub days: Option<usize>,
    //pub dt: Option<chrono::NaiveDate>,
    pub hour: Option<bool>,
    // pub lang: Option<String>,
    pub requests: RequestTypes,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub enum Location {
    Coordinate(f64, f64),
    City(String),
    USZip(usize),
    Post(String),
    Metar(String),
    Iata(String),
    #[default]
    Auto,
    IP(String),
    SearchID(usize),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ApiError {
    InvalidApiKey,
}

impl Display for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Coordinate(lat, lon) => write!(f, "{lat},{lon}"),
            Self::City(name) => write!(f, "{name}"),
            Self::USZip(zip) => write!(f, "{zip}"),
            Self::Post(code) => write!(f, "{code}"),
            Self::Metar(code) => write!(f, "metar:{code}"),
            Self::Iata(code) => write!(f, "iata:{code}"),
            Self::Auto => write!(f, "auto:ip"),
            Self::IP(ip) => write!(f, "{ip}"),
            Self::SearchID(id) => write!(f, "id:{id}"),
        }
    }
}

#[derive(Debug, Default)]
pub struct ApiResponse<'a> {
    pub current: Option<Result<&'a JsonValue, ApiResponseError<'a>>>,
    pub alerts: Option<Result<&'a JsonValue, ApiResponseError<'a>>>,
    pub forecast: Option<Result<&'a JsonValue, ApiResponseError<'a>>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct RequestTypes {
    pub current: bool,
    pub forecast: bool,
    pub alerts: bool,
}

pub struct Api {
    key: String,
    cache_current: Option<(JsonValue, Instant)>,
    cache_alerts: Option<(JsonValue, Instant)>,
    cache_forecast: Option<(JsonValue, Instant)>,
}

impl Api {
    pub fn new(key: String) -> Self {
        Self {
            key,
            cache_current: None,
            cache_alerts: None,
            cache_forecast: None,
        }
    }

    pub fn get_cached_alerts(&self) -> Option<&(JsonValue, Instant)> {
        self.cache_alerts.as_ref()
    }

    pub fn get_cached_current(&self) -> Option<&(JsonValue, Instant)> {
        self.cache_current.as_ref()
    }

    pub fn get_cached_forecast(&self) -> Option<&(JsonValue, Instant)> {
        self.cache_forecast.as_ref()
    }

    pub fn make_request<'a>(&'a mut self, config: &ApiRequestConfiguration) -> ApiResponse {
        /*
         * Solution bypasses safety checks unnecessarily,
         * but I'm too lazy to get it working in a better
         * way
         */
        let ptr: *mut Self = self;

        let requests = &config.requests;
        ApiResponse {
            current: requests.current.then(|| {
                match unsafe { &mut *ptr }.make_current_request(config) {
                    Ok(json) => {
                        if json["error"].is_null() {
                            Ok(json)
                        } else {
                            Err(ApiResponseError::ApiError(json))
                        }
                    }
                    Err(req_err) => Err(ApiResponseError::RequestError(req_err)),
                }
            }),

            alerts: requests.alerts.then(|| {
                match unsafe { &mut *ptr }.make_alerts_request(config) {
                    Ok(json) => {
                        if json["error"].is_null() {
                            Ok(json)
                        } else {
                            Err(ApiResponseError::ApiError(json))
                        }
                    }
                    Err(req_err) => Err(ApiResponseError::RequestError(req_err)),
                }
            }),

            forecast: requests.forecast.then(|| {
                match unsafe { &mut *ptr }.make_forecast_request(config) {
                    Ok(json) => {
                        if json["error"].is_null() {
                            Ok(json)
                        } else {
                            Err(ApiResponseError::ApiError(json))
                        }
                    }
                    Err(req_err) => Err(ApiResponseError::RequestError(req_err)),
                }
            })
        }
    }

    fn make_alerts_request(
        &mut self,
        config: &ApiRequestConfiguration,
    ) -> Result<&JsonValue, reqwest::Error> {
        let ApiRequestConfiguration { q, .. } = config;

        let request = reqwest::blocking::Client::new()
            .post("http://api.weatherapi.com/v1/alerts.json")
            .header("key", self.key.clone())
            .query(&[("q", format!("{q}"))]);

        let response = request.send().unwrap();

        let response = json::parse(response.text().unwrap().as_ref()).unwrap();

        self.cache_alerts = Some((response.clone(), Instant::now()));

        Ok(&self.cache_alerts.as_ref().unwrap().0)
    }

    fn make_current_request(
        &mut self,
        config: &ApiRequestConfiguration,
    ) -> Result<&JsonValue, reqwest::Error> {
        let ApiRequestConfiguration { q, days, hour, .. } = config;

        let mut request = reqwest::blocking::Client::new()
            .post("http://api.weatherapi.com/v1/current.json")
            .header("key", self.key.clone())
            .query(&[("q", format!("{q}"))]);

        if let Some(days) = days {
            request = request.query(&[("days", days)])
        }

        if let Some(hour) = hour {
            request = request.query(&[("hour", hour)])
        }

        let response = request.send().unwrap();

        let response = json::parse(response.text().unwrap().as_ref()).unwrap();

        self.cache_current = Some((response.clone(), Instant::now()));

        Ok(&self.cache_current.as_ref().unwrap().0)
    }

    fn make_forecast_request(
        &mut self,
        config: &ApiRequestConfiguration,
    ) -> Result<&JsonValue, reqwest::Error> {
        let ApiRequestConfiguration { q, days, .. } = config;

        let mut request = reqwest::blocking::Client::new()
            .post("http://api.weatherapi.com/v1/forecast.json")
            .header("key", self.key.clone())
            .query(&[("q", format!("{q}"))]);

        if let Some(days) = days {
            request = request.query(&[("days", days)])
        }

        let response = request.send().unwrap();

        let response = json::parse(response.text().unwrap().as_ref()).unwrap();

        self.cache_forecast = Some((response.clone(), Instant::now()));

        Ok(&self.cache_forecast.as_ref().unwrap().0)
    }
}

#[derive(Debug)]
pub enum ApiResponseError<'a> {
    RequestError(reqwest::Error),
    ApiError(&'a JsonValue),
}
