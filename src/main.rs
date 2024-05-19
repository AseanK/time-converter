use chrono::{DateTime, Local, NaiveDate};
use eframe::egui;
use egui::Widget;

use egui_extras::DatePickerButton;
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;
use tokio::runtime::Runtime;

struct MyApp {
    // Sender/Receiver for async notifications.
    tx: Sender<Value>,
    rx: Receiver<Value>,

    date: NaiveDate,
    hours: String,
    mins: String,
    from: String,
    to: String,
    res_time: String,
    res_date: String,
    pm: bool,
}

fn main() {
    let rt = Runtime::new().expect("Unable to create Runtime");

    // Enter the runtime so that `tokio::spawn` is available immediately.
    let _enter = rt.enter();

    // Execute the runtime in its own thread.
    // The future doesn't have to do anything. In this case, sleep forever.
    std::thread::spawn(move || {
        rt.block_on(async {
            loop {
                tokio::time::sleep(Duration::from_secs(3600)).await;
            }
        })
    });

    // Options for egui
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([300.0, 300.0]),
        ..Default::default()
    };

    // Run the GUI in the main thread.
    let _ = eframe::run_native(
        "Time Convertor",
        options,
        Box::new(|_cc| Box::new(MyApp::default())),
    );
}

impl Default for MyApp {
    fn default() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let local: DateTime<Local> = Local::now();
        let date = local.date_naive();
        let from = "America/Los_Angeles";
        let to = "Asia/Seoul";

        Self {
            tx,
            rx,
            date,
            hours: "12".to_owned(),
            mins: "00".to_owned(),
            from: from.to_string(),
            to: to.to_string(),
            res_time: "N/A".to_string(),
            res_date: "N/A".to_string(),
            pm: false,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Update the result with the async response.
        if let Ok(res) = self.rx.try_recv() {
            let res_date = res.clone()["date"].take().to_string();
            let parsed_date = &res_date[1..6];
            self.res_date = parsed_date.to_string();

            let res_time = res.clone()["time"].take().to_string();
            let parsed_time = &res_time[1..6];
            let time = format_res_time(parsed_time.to_string());
            self.res_time = time.to_string();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("Convert");
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.add_sized([120.0, 50.0], egui::Label::new("Choose Date: "));
                DatePickerButton::new(&mut self.date).ui(ui);
            });
            ui.horizontal(|ui| {
                ui.add_sized([120.0, 50.0], egui::Label::new("Enter Hour: "));
                ui.add_sized([20.0, 20.0], egui::TextEdit::singleline(&mut self.hours));
                ui.horizontal(|ui| ui.checkbox(&mut self.pm, "PM"));
            });

            ui.horizontal(|ui| {
                ui.add_sized([120.0, 50.0], egui::Label::new("Enter Minute: "));
                ui.add_sized([20.0, 20.0], egui::TextEdit::singleline(&mut self.mins));
            });
            let button = ui.add_sized([285.0, 20.0], egui::Button::new("Convert"));
            if button.clicked() {
                let time = format_time(self.pm, self.hours.clone(), self.mins.clone());
                let date_time = format!("{} {}", self.date, time);

                send_req(
                    self.from.clone(),
                    date_time,
                    self.to.clone(),
                    self.tx.clone(),
                    ctx.clone(),
                );
            }
            ui.separator();
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    let date = &self.date.to_string()[5..];
                    let formatted = date.replace("-", "/");
                    // ui.add_sized([120.0, 20.0], egui::Label::new("Before"));
                    ui.heading("Before");
                    ui.label(format!("{}", formatted));
                    ui.label(format!(
                        "{}:{} {}",
                        self.hours,
                        self.mins,
                        if self.pm {
                            "PM".to_string()
                        } else {
                            "AM".to_string()
                        }
                    ));
                });

                ui.add_space(70.0);
                ui.separator();

                ui.vertical(|ui| {
                    // ui.add_sized([120.0, 20.0], egui::Label::new("After"));
                    ui.heading("After");
                    ui.label(format!("{}", self.res_date));
                    ui.label(format!("{}", self.res_time));
                });
            });
        });
    }
}

fn send_req(from: String, time: String, to: String, tx: Sender<Value>, ctx: egui::Context) {
    tokio::spawn(async move {
        let mut params = HashMap::new();
        params.insert("fromTimeZone", from);
        params.insert("dateTime", time);
        params.insert("toTimeZone", to);
        params.insert("dstAmbiguity", "".to_string());

        // Send API request
        let response = Client::new()
            .post("https://timeapi.io/api/Conversion/ConvertTimeZone")
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .json(&params)
            .send()
            .await;

        match response {
            Ok(response) => {
                let body = response.text().await.unwrap();
                // Not sure if if case is neccessary
                if body == "" {
                    println!("Empty state");
                } else {
                    let mut response: serde_json::Value =
                        serde_json::from_str(&body).expect("Failed to parse response");
                    let res = response["conversionResult"].take();
                    let _ = tx.send(res);
                    ctx.request_repaint();
                }
            }
            Err(e) => {
                eprintln!("Error: {:#?}", e);
                let res = json!({
                    "date": "Error",
                    "time": "Please try again!"
                });

                let _ = tx.send(res);
                ctx.request_repaint();
            }
        }
    });
}

fn format_time(pm: bool, hour: String, minute: String) -> String {
    let mut formatted_hour: String = hour.clone();
    let mut formatted_min: String = minute.clone();

    if pm {
        let parsed: i8 = hour.parse().unwrap();
        let military = parsed + 12;
        formatted_hour = military.to_string();
    } else {
        // If time does not contain leading 0
        if hour.len() <= 1 {
            formatted_hour = format!("0{}", hour);
        }
        if minute.len() <= 1 {
            formatted_min = format!("0{}", minute);
        }
    }

    // API does not take 24. Instead use 00
    if formatted_hour == "24".to_string() {
        formatted_hour = "00".to_string()
    }

    return format!("{}:{}:00", formatted_hour, formatted_min);
}

fn format_res_time(time: String) -> String {
    let hour = &time[..2];
    let min = &time[3..];

    // Parse hour into i8 for comparison
    let hour_int: i8 = hour.parse().unwrap();
    // Edge case 00:00
    if hour_int == 0 {
        return format!("12:{} PM", min);
    }
    // Return time in AM/PM format
    if hour_int > 12 {
        let h = hour_int - 12;
        let hour = h.to_string();
        return format!("{}:{} PM", hour, min);
    } else {
        return format!("{}:{} AM", hour, min);
    }
}
