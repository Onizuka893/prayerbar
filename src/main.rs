use core::time;
use std::collections::HashMap;
use std::fs::{metadata, read_to_string, File};
use std::io::Write;
use std::thread;
use std::time::{Duration, SystemTime};

use chrono::prelude::*;
use clap::Parser;
use reqwest::blocking::Client;
use serde_json::{json, Value};

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, help = "pass a city")]
    city: Option<String>,
    #[arg(long, help = "pass a country")]
    country: Option<String>,
    #[arg(
        long,
        help = "pass a calculation method See https://aladhan.com/calculation-methods"
    )]
    method: Option<String>,
}

fn main() {
    let args = Args::parse();
    let dt = Local::now();
    let prayer_names: [&str; 7] = [
        "Fajr", "Sunrise", "Dhuhr", "Asr", "Maghrib", "Isha", "Midnight",
    ];

    let mut data = HashMap::new();
    let mut prayer_data: Vec<(&str, DateTime<FixedOffset>)> = Vec::new();

    let city = args.city.unwrap_or(String::new());
    let country = args.country.unwrap_or(String::new());
    let method = args.method.unwrap_or(String::new());
    let prayer_url = format!(
        "http://api.aladhan.com/v1/timingsByCity/{}?city={}&country={}&method={}",
        dt.format("%d-%m-%Y"),
        city,
        country,
        method
    );
    let cachefile = format!("/tmp/prayerbar-{}.json", city);

    let mut iterations = 0;
    let treshold = 20;

    let is_cache_file_recent = if let Ok(metadata) = metadata(&cachefile) {
        let ten_minutes_ago = SystemTime::now() - Duration::from_secs(600);
        metadata
            .modified()
            .map_or(false, |mod_time| mod_time > ten_minutes_ago)
    } else {
        false
    };

    let client = Client::new();
    let times = if is_cache_file_recent {
        let json_str = read_to_string(&cachefile).unwrap();
        serde_json::from_str::<serde_json::Value>(&json_str).unwrap()
    } else {
        loop {
            match client.get(&prayer_url).send() {
                Ok(response) => break response.json::<Value>().unwrap(),
                Err(_) => {
                    iterations += 1;
                    thread::sleep(time::Duration::from_millis(500 * iterations));

                    if iterations == treshold {
                        panic!("No response from endpoint!");
                    }
                }
            }
        }
    };

    if !is_cache_file_recent {
        let mut file = File::create(&cachefile)
            .expect(format!("Unable to create cache file at {}", cachefile).as_str());

        file.write_all(serde_json::to_string_pretty(&times).unwrap().as_bytes())
            .expect(format!("Unable to write cache file at {}", cachefile).as_str());
    }

    let mut tooltip = format!("Prayer times {}\n", city);
    let prayer_times_map = times["data"]["timings"].as_object().unwrap();
    for (prayer_name, prayer_time) in prayer_times_map.iter() {
        if prayer_names.contains(&prayer_name.as_str()) {
            tooltip += &format!("{} at {}\n", prayer_name, prayer_time);
            let prayer_time_value_str = prayer_time.as_str().unwrap();
            let date_time_str = format!(
                "{} {} {}",
                dt.format("%Y-%m-%d"),
                prayer_time_value_str,
                dt.format("%z")
            );
            let date_time = DateTime::parse_from_str(&date_time_str, "%Y-%m-%d %H:%M %z").unwrap();
            prayer_data.push((prayer_name.as_str(), date_time));
        }
    }
    prayer_data.push(("Current_time", dt.fixed_offset()));
    tooltip += "\n";

    let text = get_next_prayer(prayer_data);
    data.insert("text", text);
    data.insert("tooltip", tooltip);
    let json_data = json!(data);
    println!("{}", json_data);
}

fn get_next_prayer(mut times_vec: Vec<(&str, DateTime<FixedOffset>)>) -> String {
    let mut next_prayer = times_vec[0];
    times_vec.sort_by(|a, b| a.1.cmp(&b.1));
    let temp = times_vec[0];
    times_vec.push(temp);
    times_vec.remove(0);
    for (index, (prayer_name, _)) in times_vec.iter().enumerate() {
        let name = *prayer_name;
        if name.eq("Current_time") && index < times_vec.len() {
            next_prayer = times_vec[index + 1]
        }
    }
    format!("{} {}", next_prayer.0, next_prayer.1.format("%H:%M"))
}
