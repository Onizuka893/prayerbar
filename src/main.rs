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
    #[arg(long, help = "display calendar in Arabic format")]
    ar: bool,
}

const DEFAULT_RESULT: &[(&str, &str)] = &[("text", "N/A"), ("tooltip", "N/A")];

fn main() {
    let args = Args::parse();
    let dt = Local::now();

    let prayer_url = format!(
        "http://api.aladhan.com/v1/timingsByCity/{}?city={}&country={}&method={}",
        dt.format("%d-%m-%Y"),
        args.city.as_ref().unwrap_or(&String::default()),
        args.country.as_ref().unwrap_or(&String::default()),
        args.method.as_ref().unwrap_or(&String::default())
    );

    let cachefile = format!(
        "/tmp/prayerbar-{}.json",
        args.city.as_ref().unwrap_or(&String::default())
    );

    let mut iterations = 0;
    let treshold = 20;

    let is_cache_file_recent = if let Ok(metadata) = metadata(&cachefile) {
        let five_hours_ago = SystemTime::now() - Duration::from_secs(10800);
        metadata
            .modified()
            .map_or(false, |mod_time| mod_time > five_hours_ago)
    } else {
        false
    };

    let client = Client::new();
    let times = if is_cache_file_recent {
        let json_str = read_to_string(&cachefile).expect("Unable to read cache file");
        serde_json::from_str::<serde_json::Value>(&json_str).expect("Unable to parse cache file")
    } else {
        loop {
            match client.get(&prayer_url).send() {
                Ok(response) => break response.json::<Value>().expect("Unable to parse response"),
                Err(_) => {
                    iterations += 1;
                    thread::sleep(time::Duration::from_millis(500 * iterations));

                    if iterations == treshold {
                        eprintln!("Error connecting to alathan.com");
                        println!("{}", json!(DEFAULT_RESULT));
                    }
                }
            }
        }
    };

    if !is_cache_file_recent {
        let mut file = File::create(&cachefile)
            .unwrap_or_else(|_| panic!("Unable to create cache file at {}", cachefile));

        file.write_all(serde_json::to_string_pretty(&times).unwrap().as_bytes())
            .unwrap_or_else(|_| panic!("Unable to write cache file at {}", cachefile));
    }

    let data = parse_prayer_times(times, &args);

    let json_data = json!(data);
    println!("{}", json_data);
}

fn parse_prayer_times<'a>(times: Value, args: &Args) -> HashMap<&'a str, String> {
    let dt = Local::now();

    let prayer_icons = HashMap::from([
        ("Fajr", "🌄 "),
        ("Sunrise", "🌅 "),
        ("Dhuhr", "🏙️ "),
        ("Asr", "🏙️ "),
        ("Maghrib", "🌇 "),
        ("Isha", "🌃 "),
        ("Midnight", "🌃 "),
    ]);

    let mut data = HashMap::new();

    let mut prayer_data: Vec<(&str, DateTime<FixedOffset>)> = Vec::new();
    let language = {
        if args.ar {
            "ar"
        } else {
            "en"
        }
    };

    let mut tooltip = format!(
        "<b>Prayer times in {}</b>\n\n",
        args.city.as_ref().unwrap_or(&String::default())
    );
    let mut text = String::new();

    let hijri_date = times["data"]["date"]["hijri"]["date"]
        .as_str()
        .unwrap_or_else(|| {
            eprintln!("API returned invalid hijri date might be due to invalid city or country");
            "N/A"
        });
    if hijri_date.eq("N/A") {
        data.insert("text", "N/A".to_string());
        data.insert("tooltip", "N/A".to_string());
        return data;
    }
    let hijri_month_name = times["data"]["date"]["hijri"]["month"][language]
        .as_str()
        .expect("hijri month name not available");
    let hijri_weekday = times["data"]["date"]["hijri"]["weekday"][language]
        .as_str()
        .expect("hijri weekday not available");

    tooltip += format!(
        "🗓️ {} {} {}\n\n",
        hijri_date, hijri_month_name, hijri_weekday
    )
    .as_str();

    let prayer_times_map = times["data"]["timings"]
        .as_object()
        .expect("prayer timings not available");
    for (prayer_name, prayer_time) in prayer_times_map.iter() {
        if prayer_icons.contains_key(&prayer_name.as_str()) {
            let prayer_time_value_str = prayer_time.as_str().expect("prayer time not available");
            let date_time_str = format!(
                "{} {} {}",
                dt.format("%Y-%m-%d"),
                prayer_time_value_str,
                dt.format("%z")
            );
            let date_time = DateTime::parse_from_str(&date_time_str, "%Y-%m-%d %H:%M %z")
                .expect("unable to parse date time");
            prayer_data.push((prayer_name.as_str(), date_time));
        }
    }
    prayer_data.push(("Current_time", dt.fixed_offset()));

    sort_prayer_times(&mut prayer_data);
    format_prayerbar(&prayer_data, &mut tooltip, &mut text, &prayer_icons);

    data.insert("text", text);
    data.insert("tooltip", tooltip);
    data
}

fn sort_prayer_times(times_vec: &mut Vec<(&str, DateTime<FixedOffset>)>) {
    times_vec.sort_by(|a, b| a.1.cmp(&b.1));
    let temp = times_vec[0];

    //if Midnight > 00:00
    //else error???
    times_vec.push(temp);
    times_vec.remove(0);
}

fn format_prayerbar(
    times_vec: &Vec<(&str, DateTime<FixedOffset>)>,
    tooltip: &mut String,
    bar_text: &mut String,
    icons: &HashMap<&str, &str>,
) {
    for (index, (prayer_name, prayer_time)) in times_vec.iter().enumerate() {
        let name = *prayer_name;
        if name.eq("Current_time") && index <= times_vec.len() {
            *bar_text = format!(
                "🕋 {} {}",
                times_vec[index + 1].0,
                times_vec[index + 1].1.format("%H:%M")
            );
        } else {
            *tooltip += format!(
                "{}{} at {}\n",
                icons[name],
                name,
                prayer_time.format("%H:%M")
            )
            .as_str();
        }
    }
}
