extern crate reqwest;
extern crate regex;
extern crate chrono;
use std::env;

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use reqwest::Client;
use chrono::prelude::*;



struct Record {
    timestamp: DateTime<Utc>,
    arrival_times: String
}

/// Allows for instance-level timestamp. Acts as default-setter.
fn record(arrival_times: String) -> Record {
    Record{timestamp: Utc::now(), arrival_times}
}

lazy_static! {
    static ref TIMETABLE: Mutex<HashMap<String, Record>> = {
        let mut _map = HashMap::new();
        Mutex::new(_map)
    };
}

lazy_static! { static ref NON_STOPS: Mutex<HashSet<String>> = {
        let mut _set = HashSet::new();
        Mutex::new(_set)
        };
}

macro_rules! regex { ($re:expr) => { ::regex::Regex::new($re).unwrap() } }

// String formats API KEY and STOP ID into URL BASE. Returns complete url for get request.
fn construct_url(stop_id: String) -> String {
    let api_key: String = env::var("BUSPLUG_API_KEY").expect("Missing env var `MY_KEY`");
    format!("http://www.ctabustracker.com/bustime/api/v2/getpredictions?key={}&stpid={}", api_key, stop_id)
}

fn get_string_from_match<'a>(capture: &'a regex::Captures) -> &'a str {
    match capture.get(1) {
        Some(target) => target.as_str(),
        None => ""
    }

}

fn get_stop_name(xml: &str) -> String {
    let pattern_match = regex!(r"<stpnm>(.*)</stpnm>").captures(xml).unwrap();
    let match_string: String = get_string_from_match(&pattern_match)
        .replace("&amp;", "&")
        .to_string();
    match_string
}

/// Written iteratively because API can return varying numbers of arrival estimations.
fn get_arrival_times(xml: &str) -> String {
    let mut arrival_times: String = String::new();

    for pattern_match in regex!(r"<prdctdn>(.*)</prdctdn>").captures_iter(xml) {
        let arrival_time: &str = get_string_from_match(&pattern_match);
        arrival_times.push_str(&format!("{}...", arrival_time));
    }

    arrival_times

}

fn contains_tags(xml: &str, tags: &str) -> bool {
    xml.contains(&format!("<{}>", tags)) & xml.contains(&format!("</{}>", tags))
}

/// Handles case where response contains arrival predictions.
/// Populates TIMETABLE with arrival message for provided stop.
fn handle_predictions(stop_id: String, xml: &str) -> String {
    let result: String = format!("{}  -  {}", get_stop_name(xml), get_arrival_times(xml));

    let mut map = TIMETABLE.lock().unwrap();
    map.insert(stop_id, record(result.clone()));

    result
}

/// Handles case where response does not contain arrival predictions.
/// Populates NON_STOPS if API says is invalid.
/// Populates TIMETABLE if API says no incoming arrivals.
fn handle_message(stop_id: String, xml: &str) -> String {
    let pattern_match = regex!(r"<msg>(.*)</msg>").captures(xml).unwrap();
    let message: String = get_string_from_match(&pattern_match)
        .replace("&amp;", "&")
        .to_string();

    if message.to_lowercase().contains("no data found for parameter") {
        let mut set = NON_STOPS.lock().unwrap();
        set.insert(stop_id);
        "Not a valid stop id".to_string()
    }

    else {
        let mut map = TIMETABLE.lock().unwrap();
        map.insert(stop_id, record(message.clone()));
        message
    }
}

/// Response can come in two formats: one containing the arrival times, the other containing a
/// message that implies no incoming arrivals or that provided id is not valid.
fn handle_response(stop_id: String, mut response: reqwest::Response) -> String {
    let xml: &str = &response.text().expect("Unable to get text from response.");

    if contains_tags(xml, &"prdctdn") {
        handle_predictions(stop_id, xml)
    } else if contains_tags(xml, &"msg") {
        handle_message(stop_id, xml)
    } else {
        "Unknown response format".to_string()
    }
}

/// Sends get request to API and handles response.
fn request_arrival_times(stop_id: &str) -> String {
    let url: &str = &construct_url(stop_id.to_string());
    let send_result: Result<reqwest::Response, reqwest::Error> = Client::new().get(url).send();

    match send_result {
        Ok(response) => handle_response(stop_id.to_string(), response),
        Err(_fail) => "There was an error sending the request".to_string()
    }
}

/// Checks set of stop ids that API has said are bad.
fn is_known_non_stop(stop_id: &str) -> bool {
    let mut set = NON_STOPS.lock().unwrap();
    set.contains(stop_id)
}

fn is_recent(last_record_timestamp: DateTime<Utc>) -> bool {
    (Utc::now() - last_record_timestamp).num_seconds() < 60
}

/// Logic to determine if last entry exists in timetable and is recent for given stop id.
/// Serves as fast-lookup for consecutive same requests and throttle on API.
fn is_recent_timetable_entry(stop_id: &str) -> bool {
    let mut use_timetable: bool = false;
    let mut map = TIMETABLE.lock().unwrap();

    if map.contains_key(stop_id) {
        let last_record: &Record = map.get(stop_id).unwrap();
        if is_recent(last_record.timestamp) {
            use_timetable = true
        }
    }

    use_timetable

}

/// Returns last timetable entry for stop.
fn read_timetable_entry(stop_id: &str) -> String {
    let mut map = TIMETABLE.lock().unwrap();
    let timetable_entry: &str = &map.get(stop_id).unwrap().arrival_times;
    timetable_entry.to_string()
}

/// Core functionality of application. Will try to read from memory if recent entry exists
/// otherwise will send get request.
pub fn run(stop_id: &str) -> String {
    if is_recent_timetable_entry(stop_id) {
        read_timetable_entry(stop_id)
    }
    else if is_known_non_stop(stop_id) {
        "Not a valid stop id".to_string()
    }
    else {
        request_arrival_times(stop_id)
    }
}
