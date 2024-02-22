extern crate reqwest;
extern crate serde_json;
extern crate tokio;
extern crate serde;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::{self, error};
use std::thread::sleep;
use std::time::Duration;
use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{MapVirtualKeyW, SendInput, SetActiveWindow, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, MAPVK_VK_TO_CHAR, VIRTUAL_KEY};
use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowTextW, IsWindowVisible, ShowWindow, SW_SHOWMAXIMIZED};
use std::mem::size_of;
use std::collections::HashMap;

// Title of the app we want to interact with. We may need part of 
const APP_TITLE: &str = "VisualBoy";
// Result of handle. THIS IS ANTI PATTERN TO DECLARE AS GLOBAL CONST, but there currently no better way to fetch result from external API enum_windows_callback
static mut FOUND_HWND: HWND = HWND(0);
static mut COMMENTS_MAP: BTreeMap<i64, Comment> = BTreeMap::new();

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Comment {
    pub id: String,
    pub message: String,
    pub created_time: String
}

// Input token from https://developers.facebook.com/tools/explorer/
const ACCESS_TOKEN: &str = "";

// TODO: add ffmpeg as option if users want to include running ffmpeg together with the app

// Bring the window to top so sendinput will work
fn bring_window_to_top(hwnd: HWND) {
    unsafe {
        println!("{:?}", hwnd);
        ShowWindow(hwnd, SW_SHOWMAXIMIZED);
        let error = windows::Win32::Foundation::GetLastError();
        println!("Err after ShowWindow {:?}", error); 
        SetActiveWindow(hwnd);
        let error = windows::Win32::Foundation::GetLastError();
        println!("Err after SetActiveWindow {:?}", error);  
    }
}

// Send key as VIRTUAL_KEY to window with hwnd handle
fn send_key(hwnd: HWND, key: VIRTUAL_KEY) {
    println!("Sending key: {:?}", key);
    bring_window_to_top(hwnd);
        unsafe {
            let mut pinputs = INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: key,
                        wScan: 0,
                        dwFlags: windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0),
                        time: 0,
                        dwExtraInfo: 0,
                    }
                }
            };
            // Send key down
            SendInput(
            &[pinputs as INPUT],
            size_of::<INPUT>()
                .try_into()
                .expect("Could not convert the size of INPUT to i32"),
            );
            sleep(Duration::from_millis(50));

            pinputs.Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;

            // Send key up
            SendInput(
            &[pinputs as INPUT],
            size_of::<INPUT>()
                .try_into()
                .expect("Could not convert the size of INPUT to i32"),
            );
        }
}

// Pull latest comments periodically
async fn latest_comment(access_token: &str) -> Result<Vec<Comment>, Box<dyn error::Error>> {
    let base_url = "https://graph.facebook.com/me?fields=id,name,posts.limit(1){comments.order(reverse_chronological).limit(20)}";
    let mut params = HashMap::new();
    params.insert("access_token", access_token);

    let client = reqwest::Client::new();
    let res = client.get(base_url).query(&params).send().await?;
    let data: serde_json::Value = res.json().await?;
    let comments_json = &data["posts"]["data"][0]["comments"]["data"];
    let data: Vec<Comment>  = serde_json::from_value(comments_json.clone())?;

    let mut new_comments: Vec<Comment> = vec![];

    for i in 0..data.len() {
        // comment id is a string with underscore, split _ and get the true id
        let comment = &data[i];
        let id_str = comment.id.split('_').nth(1);
        if !id_str.is_some() {
            continue;
        }
        // Only get new comments that are not in the map
        let id: i64 = id_str.unwrap().parse().expect("Failed to parse a number");
        unsafe {
            if !COMMENTS_MAP.contains_key(&id) {
                new_comments.push(comment.clone());
                COMMENTS_MAP.insert(id, comment.clone()) ;
            }
        }
    }
    Ok(new_comments)
}

// Parse command to corresponding VIRTUAL_KEY. Currently get the first character of comment
fn parse (command : String ) -> VIRTUAL_KEY {
    let first_char = command.to_uppercase().as_bytes()[0];
    let vk_key = unsafe { MapVirtualKeyW(first_char as u32, MAPVK_VK_TO_CHAR) };
    VIRTUAL_KEY(vk_key as u16)
}

// Scan window and check if title contains APP_TITLE
fn scan_window() {
    unsafe extern "system" fn enum_windows_callback(hwnd: HWND, lparam:LPARAM) -> BOOL {
        const MAX_TITLE_LENGTH: usize = 256;
        let mut title = [0u16; MAX_TITLE_LENGTH];
        let lpstring:&mut[u16] = title.as_mut();

        if IsWindowVisible(hwnd) == true && GetWindowTextW(hwnd, lpstring) > 0 {
            let title_str = String::from_utf16_lossy(&title);

            if title_str.contains(APP_TITLE) {
                println!("Found window with title: {:?}: {:?}", title_str, hwnd);
                FOUND_HWND = hwnd;
            }
        }

        windows::Win32::Foundation::BOOL(1)
    }

    unsafe {
        let _ = EnumWindows(Some(enum_windows_callback), LPARAM(0));
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn error::Error>> {
    scan_window();
    loop {
        match latest_comment(&ACCESS_TOKEN).await {
            Ok(comments) => {
                scan_window();
                unsafe {
                    let hwnd = FOUND_HWND;
                    if hwnd != HWND(0) {
                        for single_comment in comments {
                            println!("Comment: {:?}", single_comment.message);
                            send_key(hwnd, parse(single_comment.message));
                        }
                    }
                }
            },
            Err(err) => print!("Error: {}", err),
        }
        sleep(Duration::from_millis(1000));
    }

    Ok(())
}
