//! Boa JavaScript runtime integration for TUI automation scripting.
//!
//! This module provides a JavaScript execution environment with a `tui` global object
//! that exposes TUI automation methods like `tui.text()`, `tui.sendText()`, etc.
//!
//! It also provides a `console` object with `log`, `warn`, `error`, `info`, `debug` methods
//! that capture messages for later retrieval.

use boa_engine::{
    builtins::promise::PromiseState,
    js_string,
    native_function::NativeFunction,
    object::{builtins::JsArray, FunctionObjectBuilder, JsObject},
    property::Attribute,
    Context, JsArgs, JsResult, JsString, JsValue, Source,
};
use base64::Engine;
use std::cell::RefCell;
use std::fs;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};
use tui_driver::{snapshot::Snapshot, Key, Row, Signal, Span, TuiDriver};

use crate::server::ConsoleEntry;

/// Execute a JavaScript script with access to TUI automation functions.
///
/// The script has access to a global `tui` object with the following methods:
///
/// Text/Input:
/// - `tui.text()` - Returns the current screen text
/// - `tui.sendText(text)` - Sends text to the terminal
/// - `tui.pressKey(key)` - Presses a key (e.g., "Enter", "Ctrl+c")
/// - `tui.pressKeys(keys)` - Presses multiple keys in sequence
///
/// Mouse:
/// - `tui.clickAt(x, y)` - Clicks at the specified coordinates
/// - `tui.click(ref)` - Clicks on element by reference ID
/// - `tui.doubleClick(ref)` - Double-clicks on element by reference ID
/// - `tui.rightClick(ref)` - Right-clicks on element by reference ID
/// - `tui.hover(ref)` - Hovers over element by reference ID
/// - `tui.drag(startRef, endRef)` - Drags from one element to another
///
/// Wait:
/// - `tui.waitForText(text, timeoutMs?)` - Waits for text to appear, returns boolean
/// - `tui.waitForIdle(timeoutMs?, idleMs?)` - Waits for screen to settle, returns boolean
///
/// Snapshot:
/// - `tui.snapshot()` - Returns an accessibility snapshot as a JavaScript object
/// - `tui.screenshot(filename?)` - Takes a screenshot and saves to file, returns the file path
///
/// Control:
/// - `tui.resize(cols, rows)` - Resizes the terminal
/// - `tui.sendSignal(signal)` - Sends a signal (SIGINT, SIGTERM, etc.)
///
/// Debug:
/// - `tui.getScrollback()` - Returns number of lines scrolled off screen
/// - `tui.getInput(chars?)` - Returns raw input buffer (escape sequences sent to process)
/// - `tui.getOutput(chars?)` - Returns raw output buffer (PTY output)
///
/// Console:
/// - `console.log(...)` - Log message at "log" level
/// - `console.info(...)` - Log message at "info" level
/// - `console.warn(...)` - Log message at "warn" level
/// - `console.error(...)` - Log message at "error" level
/// - `console.debug(...)` - Log message at "debug" level
///
/// # Arguments
///
/// * `driver` - Reference to the TuiDriver instance
/// * `code` - JavaScript code to execute
///
/// # Returns
///
/// Returns a tuple of (result_string, console_logs) where result_string is the
/// string representation of the last evaluated expression, and console_logs
/// contains all messages logged via console methods.
pub async fn execute_script(
    driver: &TuiDriver,
    code: &str,
) -> Result<(String, Vec<ConsoleEntry>), String> {
    // We need to run Boa in a blocking context because:
    // 1. Boa's Context is not Send (uses Rc internally)
    // 2. The wait methods use Handle::current().block_on() for async driver calls
    // 3. block_in_place tells tokio we're blocking, preventing deadlocks
    //
    // NOTE: block_in_place requires multi-threaded tokio runtime.
    // Tests must use #[tokio::test(flavor = "multi_thread")].
    let driver_ptr = driver as *const TuiDriver;
    let code = code.to_string();

    tokio::task::block_in_place(move || {
        execute_script_blocking(driver_ptr, &code)
    })
}

/// Internal synchronous script execution.
fn execute_script_blocking(
    driver_ptr: *const TuiDriver,
    code: &str,
) -> Result<(String, Vec<ConsoleEntry>), String> {
    // Create a new JavaScript context
    let mut context = Context::default();

    // Create a shared vector for capturing console logs
    // Using Rc<RefCell<>> because Boa's Context is single-threaded
    let logs: Rc<RefCell<Vec<ConsoleEntry>>> = Rc::new(RefCell::new(Vec::new()));

    // Create the tui object
    let tui_object = create_tui_object(&mut context, driver_ptr)
        .map_err(|e| format!("Failed to create tui object: {}", e))?;

    // Register the tui object as a global property
    context
        .register_global_property(
            js_string!("tui"),
            tui_object,
            Attribute::READONLY | Attribute::NON_ENUMERABLE | Attribute::PERMANENT,
        )
        .map_err(|e| format!("Failed to register tui global: {}", e))?;

    // Create the console object
    let console_object = create_console_object(&mut context, logs.clone())
        .map_err(|e| format!("Failed to create console object: {}", e))?;

    // Register the console object as a global property
    context
        .register_global_property(
            js_string!("console"),
            console_object,
            Attribute::READONLY | Attribute::NON_ENUMERABLE | Attribute::PERMANENT,
        )
        .map_err(|e| format!("Failed to register console global: {}", e))?;

    // Execute the JavaScript code
    let result = context
        .eval(Source::from_bytes(code))
        .map_err(|e| format!("JavaScript error: {}", e))?;

    // Run the job queue to process any pending Promise jobs
    context
        .run_jobs()
        .map_err(|e| format!("Job queue error: {}", e))?;

    // If the result is a Promise, wait for it to settle
    let final_result = if let Some(promise) = result.as_promise() {
        // Poll until the Promise is settled
        const MAX_ITERATIONS: u32 = 100000; // Safety limit (100k iterations)
        let mut iterations = 0;

        loop {
            match promise.state() {
                PromiseState::Fulfilled(value) => break value,
                PromiseState::Rejected(err) => {
                    return Err(format!("Promise rejected: {:?}", err));
                }
                PromiseState::Pending => {
                    iterations += 1;
                    if iterations > MAX_ITERATIONS {
                        return Err("Promise did not settle within iteration limit".to_string());
                    }

                    // Small sleep to avoid busy-waiting
                    std::thread::sleep(std::time::Duration::from_millis(1));

                    // Run more jobs (may have new jobs from async callbacks)
                    context
                        .run_jobs()
                        .map_err(|e| format!("Job queue error: {}", e))?;
                }
            }
        }
    } else {
        result
    };

    // Convert result to string
    let result_str = final_result
        .to_string(&mut context)
        .map_err(|e| format!("Failed to convert result: {}", e))?;

    // Extract the captured logs
    let captured_logs = Rc::try_unwrap(logs)
        .map(|cell| cell.into_inner())
        .unwrap_or_else(|rc| rc.borrow().clone());

    Ok((result_str.to_std_string_escaped(), captured_logs))
}

/// Create the `tui` JavaScript object with all automation methods.
fn create_tui_object(context: &mut Context, driver_ptr: *const TuiDriver) -> JsResult<JsValue> {
    // Create an empty object for tui
    let tui_obj = boa_engine::JsObject::with_null_proto();

    // Text/Input methods
    let text_fn = create_text_method(context, driver_ptr);
    tui_obj.set(js_string!("text"), text_fn, false, context)?;

    let send_text_fn = create_send_text_method(context, driver_ptr);
    tui_obj.set(js_string!("sendText"), send_text_fn, false, context)?;

    let press_key_fn = create_press_key_method(context, driver_ptr);
    tui_obj.set(js_string!("pressKey"), press_key_fn, false, context)?;

    let press_keys_fn = create_press_keys_method(context, driver_ptr);
    tui_obj.set(js_string!("pressKeys"), press_keys_fn, false, context)?;

    // Mouse methods
    let click_at_fn = create_click_at_method(context, driver_ptr);
    tui_obj.set(js_string!("clickAt"), click_at_fn, false, context)?;

    let click_fn = create_click_method(context, driver_ptr);
    tui_obj.set(js_string!("click"), click_fn, false, context)?;

    let double_click_fn = create_double_click_method(context, driver_ptr);
    tui_obj.set(js_string!("doubleClick"), double_click_fn, false, context)?;

    let right_click_fn = create_right_click_method(context, driver_ptr);
    tui_obj.set(js_string!("rightClick"), right_click_fn, false, context)?;

    let hover_fn = create_hover_method(context, driver_ptr);
    tui_obj.set(js_string!("hover"), hover_fn, false, context)?;

    let drag_fn = create_drag_method(context, driver_ptr);
    tui_obj.set(js_string!("drag"), drag_fn, false, context)?;

    // Wait methods
    let wait_for_text_fn = create_wait_for_text_method(context, driver_ptr);
    tui_obj.set(js_string!("waitForText"), wait_for_text_fn, false, context)?;

    let wait_for_idle_fn = create_wait_for_idle_method(context, driver_ptr);
    tui_obj.set(js_string!("waitForIdle"), wait_for_idle_fn, false, context)?;

    // Snapshot method
    let snapshot_fn = create_snapshot_method(context, driver_ptr);
    tui_obj.set(js_string!("snapshot"), snapshot_fn, false, context)?;

    // Screenshot method (saves to file)
    let screenshot_fn = create_screenshot_method(context, driver_ptr);
    tui_obj.set(js_string!("screenshot"), screenshot_fn, false, context)?;

    // Control methods
    let resize_fn = create_resize_method(context, driver_ptr);
    tui_obj.set(js_string!("resize"), resize_fn, false, context)?;

    let send_signal_fn = create_send_signal_method(context, driver_ptr);
    tui_obj.set(js_string!("sendSignal"), send_signal_fn, false, context)?;

    // Debug methods
    let get_scrollback_fn = create_get_scrollback_method(context, driver_ptr);
    tui_obj.set(js_string!("getScrollback"), get_scrollback_fn, false, context)?;

    let get_input_fn = create_get_input_method(context, driver_ptr);
    tui_obj.set(js_string!("getInput"), get_input_fn, false, context)?;

    let get_output_fn = create_get_output_method(context, driver_ptr);
    tui_obj.set(js_string!("getOutput"), get_output_fn, false, context)?;

    Ok(tui_obj.into())
}

/// Create the `console` JavaScript object with log capture methods.
///
/// The console object provides `log`, `warn`, `error`, `info`, and `debug` methods
/// that capture messages to a shared Vec for later retrieval.
///
/// Each method accepts any number of arguments, converts them to strings,
/// joins them with spaces, and stores them with the appropriate log level.
fn create_console_object(
    context: &mut Context,
    logs: Rc<RefCell<Vec<ConsoleEntry>>>,
) -> JsResult<JsValue> {
    let console_obj = JsObject::with_null_proto();

    // Create method for each log level
    let log_fn = create_console_method(context, logs.clone(), "log");
    console_obj.set(js_string!("log"), log_fn, false, context)?;

    let info_fn = create_console_method(context, logs.clone(), "info");
    console_obj.set(js_string!("info"), info_fn, false, context)?;

    let warn_fn = create_console_method(context, logs.clone(), "warn");
    console_obj.set(js_string!("warn"), warn_fn, false, context)?;

    let error_fn = create_console_method(context, logs.clone(), "error");
    console_obj.set(js_string!("error"), error_fn, false, context)?;

    let debug_fn = create_console_method(context, logs, "debug");
    console_obj.set(js_string!("debug"), debug_fn, false, context)?;

    Ok(console_obj.into())
}

/// Create a console method (log, warn, error, info, debug) that captures messages.
///
/// The method accepts any number of arguments, converts each to a string,
/// joins them with spaces, and stores the result in the shared logs vector.
///
/// # Safety
///
/// This uses a raw pointer to the RefCell, which is safe because:
/// 1. The Rc<RefCell<Vec<ConsoleEntry>>> is valid for the entire duration of script execution
/// 2. All JavaScript execution happens synchronously within execute_script
/// 3. The Boa context is single-threaded
fn create_console_method(
    context: &mut Context,
    logs: Rc<RefCell<Vec<ConsoleEntry>>>,
    level: &'static str,
) -> JsValue {
    // Get a raw pointer to the inner RefCell. This is safe because:
    // 1. The Rc is passed by value and we hold a reference to it
    // 2. The logs Rc in execute_script outlives all JS execution
    // 3. We use Rc::as_ptr which doesn't affect reference counting
    let logs_ptr = Rc::as_ptr(&logs) as usize;

    FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            // SAFETY: logs_ptr points to a RefCell that is valid for the duration
            // of script execution, and all JS execution is synchronous
            let logs_ref = unsafe { &*(logs_ptr as *const RefCell<Vec<ConsoleEntry>>) };

            // Convert all arguments to strings and join with space
            let mut parts = Vec::with_capacity(args.len());
            for arg in args.iter() {
                let s = arg.to_string(ctx)?.to_std_string_escaped();
                parts.push(s);
            }
            let message = parts.join(" ");

            // Store the log entry
            logs_ref.borrow_mut().push(ConsoleEntry {
                level: level.to_string(),
                message,
            });

            Ok(JsValue::undefined())
        }),
    )
    .name(js_string!(level))
    .length(0) // Variable arguments
    .build()
    .into()
}

/// Create the tui.text() method that returns screen text.
fn create_text_method(context: &mut Context, driver_ptr: *const TuiDriver) -> JsValue {
    // We use a raw pointer here because the driver lifetime is guaranteed
    // to outlive the script execution (synchronous execution in execute_script)
    let ptr = driver_ptr as usize;

    FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_copy_closure(move |_this, _args, _ctx| {
            // SAFETY: ptr was created from a valid reference that outlives this closure
            let driver = unsafe { &*(ptr as *const TuiDriver) };
            let text = driver.text();
            Ok(JsValue::from(JsString::from(text.as_str())))
        }),
    )
    .name(js_string!("text"))
    .length(0)
    .build()
    .into()
}

/// Create the tui.sendText(text) method.
fn create_send_text_method(context: &mut Context, driver_ptr: *const TuiDriver) -> JsValue {
    let ptr = driver_ptr as usize;

    FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            // Get the text argument
            let text = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_escaped();

            // SAFETY: ptr was created from a valid reference that outlives this closure
            let driver = unsafe { &*(ptr as *const TuiDriver) };

            match driver.send_text(&text) {
                Ok(()) => Ok(JsValue::undefined()),
                Err(e) => Err(boa_engine::JsError::from_opaque(JsValue::from(
                    JsString::from(format!("sendText error: {}", e).as_str()),
                ))),
            }
        }),
    )
    .name(js_string!("sendText"))
    .length(1)
    .build()
    .into()
}

/// Create the tui.pressKey(key) method.
fn create_press_key_method(context: &mut Context, driver_ptr: *const TuiDriver) -> JsValue {
    let ptr = driver_ptr as usize;

    FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            // Get the key argument
            let key_str = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_escaped();

            // Parse the key
            let key = Key::parse(&key_str).map_err(|e| {
                boa_engine::JsError::from_opaque(JsValue::from(JsString::from(
                    format!("Invalid key '{}': {}", key_str, e).as_str(),
                )))
            })?;

            // SAFETY: ptr was created from a valid reference that outlives this closure
            let driver = unsafe { &*(ptr as *const TuiDriver) };

            match driver.press_key(&key) {
                Ok(()) => Ok(JsValue::undefined()),
                Err(e) => Err(boa_engine::JsError::from_opaque(JsValue::from(
                    JsString::from(format!("pressKey error: {}", e).as_str()),
                ))),
            }
        }),
    )
    .name(js_string!("pressKey"))
    .length(1)
    .build()
    .into()
}

/// Create the tui.clickAt(x, y) method.
fn create_click_at_method(context: &mut Context, driver_ptr: *const TuiDriver) -> JsValue {
    let ptr = driver_ptr as usize;

    FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            // Get x and y arguments
            let x = args.get_or_undefined(0).to_u32(ctx)? as u16;
            let y = args.get_or_undefined(1).to_u32(ctx)? as u16;

            // SAFETY: ptr was created from a valid reference that outlives this closure
            let driver = unsafe { &*(ptr as *const TuiDriver) };

            match driver.click_at(x, y) {
                Ok(()) => Ok(JsValue::undefined()),
                Err(e) => Err(boa_engine::JsError::from_opaque(JsValue::from(
                    JsString::from(format!("clickAt error: {}", e).as_str()),
                ))),
            }
        }),
    )
    .name(js_string!("clickAt"))
    .length(2)
    .build()
    .into()
}

/// Convert a Span to a JavaScript object.
///
/// Only includes optional fields if they have truthy values.
fn span_to_js_object(span: &Span, context: &mut Context) -> JsResult<JsObject> {
    let obj = JsObject::with_null_proto();

    // Required fields
    obj.set(
        js_string!("ref"),
        JsValue::from(JsString::from(span.ref_id.as_str())),
        false,
        context,
    )?;
    obj.set(
        js_string!("text"),
        JsValue::from(JsString::from(span.text.as_str())),
        false,
        context,
    )?;
    obj.set(
        js_string!("x"),
        JsValue::from(span.x as i32),
        false,
        context,
    )?;
    obj.set(
        js_string!("y"),
        JsValue::from(span.y as i32),
        false,
        context,
    )?;
    obj.set(
        js_string!("width"),
        JsValue::from(span.width as i32),
        false,
        context,
    )?;

    // Optional boolean fields - only include if true
    if span.bold == Some(true) {
        obj.set(js_string!("bold"), JsValue::from(true), false, context)?;
    }
    if span.italic == Some(true) {
        obj.set(js_string!("italic"), JsValue::from(true), false, context)?;
    }
    if span.underline == Some(true) {
        obj.set(
            js_string!("underline"),
            JsValue::from(true),
            false,
            context,
        )?;
    }
    if span.inverse == Some(true) {
        obj.set(js_string!("inverse"), JsValue::from(true), false, context)?;
    }
    if span.strikethrough == Some(true) {
        obj.set(
            js_string!("strikethrough"),
            JsValue::from(true),
            false,
            context,
        )?;
    }

    // Optional string fields - only include if Some
    if let Some(ref fg) = span.fg {
        obj.set(
            js_string!("fg"),
            JsValue::from(JsString::from(fg.as_str())),
            false,
            context,
        )?;
    }
    if let Some(ref bg) = span.bg {
        obj.set(
            js_string!("bg"),
            JsValue::from(JsString::from(bg.as_str())),
            false,
            context,
        )?;
    }
    if let Some(ref underline_style) = span.underline_style {
        obj.set(
            js_string!("underline_style"),
            JsValue::from(JsString::from(underline_style.as_str())),
            false,
            context,
        )?;
    }
    if let Some(ref blink) = span.blink {
        obj.set(
            js_string!("blink"),
            JsValue::from(JsString::from(blink.as_str())),
            false,
            context,
        )?;
    }
    if let Some(ref link) = span.link {
        obj.set(
            js_string!("link"),
            JsValue::from(JsString::from(link.as_str())),
            false,
            context,
        )?;
    }
    if let Some(ref image) = span.image {
        obj.set(
            js_string!("image"),
            JsValue::from(JsString::from(image.as_str())),
            false,
            context,
        )?;
    }
    if let Some(ref image_size) = span.image_size {
        obj.set(
            js_string!("image_size"),
            JsValue::from(JsString::from(image_size.as_str())),
            false,
            context,
        )?;
    }

    Ok(obj)
}

/// Convert a Row to a JavaScript object.
fn row_to_js_object(row: &Row, context: &mut Context) -> JsResult<JsObject> {
    let obj = JsObject::with_null_proto();

    // Row number
    obj.set(
        js_string!("row_number"),
        JsValue::from(row.row as i32),
        false,
        context,
    )?;

    // Convert spans to JS array
    let spans_array = JsArray::new(context);
    for span in &row.spans {
        let span_obj = span_to_js_object(span, context)?;
        spans_array.push(JsValue::from(span_obj), context)?;
    }
    obj.set(js_string!("spans"), JsValue::from(spans_array), false, context)?;

    Ok(obj)
}

/// Convert a Snapshot to a JavaScript object.
fn snapshot_to_js_object(snapshot: &Snapshot, context: &mut Context) -> JsResult<JsObject> {
    let obj = JsObject::with_null_proto();

    // Convert rows to JS array
    let rows_array = JsArray::new(context);
    for row in &snapshot.rows {
        let row_obj = row_to_js_object(row, context)?;
        rows_array.push(JsValue::from(row_obj), context)?;
    }
    obj.set(js_string!("rows"), JsValue::from(rows_array), false, context)?;

    // Convert flat spans list to JS array
    let spans_array = JsArray::new(context);
    for span in &snapshot.spans {
        let span_obj = span_to_js_object(span, context)?;
        spans_array.push(JsValue::from(span_obj), context)?;
    }
    obj.set(js_string!("spans"), JsValue::from(spans_array), false, context)?;

    // Add span count
    obj.set(
        js_string!("span_count"),
        JsValue::from(snapshot.spans.len() as i32),
        false,
        context,
    )?;

    Ok(obj)
}

/// Create the tui.snapshot() method that returns a structured JavaScript object.
fn create_snapshot_method(context: &mut Context, driver_ptr: *const TuiDriver) -> JsValue {
    let ptr = driver_ptr as usize;

    FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_copy_closure(move |_this, _args, ctx| {
            // SAFETY: ptr was created from a valid reference that outlives this closure
            let driver = unsafe { &*(ptr as *const TuiDriver) };
            let snapshot = driver.snapshot();
            let js_obj = snapshot_to_js_object(&snapshot, ctx)?;
            Ok(JsValue::from(js_obj))
        }),
    )
    .name(js_string!("snapshot"))
    .length(0)
    .build()
    .into()
}

/// Create the tui.screenshot(filename?) method that saves a screenshot to file.
///
/// If filename is not provided, generates one with timestamp.
/// Creates directory `/tmp/tui-screenshots/<session-id>/` if it does not exist.
/// Returns the full file path of the saved screenshot.
fn create_screenshot_method(context: &mut Context, driver_ptr: *const TuiDriver) -> JsValue {
    let ptr = driver_ptr as usize;

    FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            // SAFETY: ptr was created from a valid reference that outlives this closure
            let driver = unsafe { &*(ptr as *const TuiDriver) };

            // Get session ID for directory path
            let session_id = driver.session_id();

            // Get optional filename argument
            let filename_arg = args.get_or_undefined(0);
            let filename = if filename_arg.is_undefined() || filename_arg.is_null() {
                // Generate filename with timestamp
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or(0);
                format!("screenshot-{}.png", timestamp)
            } else {
                let name = filename_arg.to_string(ctx)?.to_std_string_escaped();

                // Sanitize: reject filenames with path separators or parent directory references
                if name.contains('/') || name.contains('\\') || name.contains("..") {
                    return Err(boa_engine::JsError::from_opaque(JsValue::from(
                        JsString::from("Filename cannot contain path separators or '..'"),
                    )));
                }

                // Ensure filename ends with .png
                if name.ends_with(".png") {
                    name
                } else {
                    format!("{}.png", name)
                }
            };

            // Create directory path
            let dir_path = format!("/tmp/tui-screenshots/{}", session_id);

            // Create directory if it does not exist
            if let Err(e) = fs::create_dir_all(&dir_path) {
                return Err(boa_engine::JsError::from_opaque(JsValue::from(
                    JsString::from(format!("Failed to create screenshot directory: {}", e).as_str()),
                )));
            }

            // Take screenshot
            let screenshot = driver.screenshot();

            // Decode base64 data to raw bytes
            let png_bytes = base64::engine::general_purpose::STANDARD
                .decode(&screenshot.data)
                .map_err(|e| {
                    boa_engine::JsError::from_opaque(JsValue::from(JsString::from(
                        format!("Failed to decode screenshot data: {}", e).as_str(),
                    )))
                })?;

            // Full file path
            let file_path = format!("{}/{}", dir_path, filename);

            // Write PNG bytes to file
            if let Err(e) = fs::write(&file_path, png_bytes) {
                return Err(boa_engine::JsError::from_opaque(JsValue::from(
                    JsString::from(format!("Failed to write screenshot file: {}", e).as_str()),
                )));
            }

            Ok(JsValue::from(JsString::from(file_path.as_str())))
        }),
    )
    .name(js_string!("screenshot"))
    .length(0)
    .build()
    .into()
}

/// Create the tui.pressKeys(keys) method for pressing multiple keys in sequence.
fn create_press_keys_method(context: &mut Context, driver_ptr: *const TuiDriver) -> JsValue {
    let ptr = driver_ptr as usize;

    FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            // Get the keys array argument
            let keys_arg = args.get_or_undefined(0);
            let keys_obj = keys_arg.as_object().ok_or_else(|| {
                boa_engine::JsError::from_opaque(JsValue::from(JsString::from(
                    "pressKeys requires an array of key strings",
                )))
            })?;

            // Get length of array
            let length_val = keys_obj.get(js_string!("length"), ctx)?;
            let length = length_val.to_u32(ctx)? as usize;

            // Parse all keys first
            let mut keys = Vec::with_capacity(length);
            for i in 0..length {
                let key_val = keys_obj.get(i as u32, ctx)?;
                let key_str = key_val.to_string(ctx)?.to_std_string_escaped();
                let key = Key::parse(&key_str).map_err(|e| {
                    boa_engine::JsError::from_opaque(JsValue::from(JsString::from(
                        format!("Invalid key '{}': {}", key_str, e).as_str(),
                    )))
                })?;
                keys.push(key);
            }

            // SAFETY: ptr was created from a valid reference that outlives this closure
            let driver = unsafe { &*(ptr as *const TuiDriver) };

            match driver.press_keys(&keys) {
                Ok(()) => Ok(JsValue::undefined()),
                Err(e) => Err(boa_engine::JsError::from_opaque(JsValue::from(
                    JsString::from(format!("pressKeys error: {}", e).as_str()),
                ))),
            }
        }),
    )
    .name(js_string!("pressKeys"))
    .length(1)
    .build()
    .into()
}

/// Create the tui.click(ref) method for clicking by reference ID.
fn create_click_method(context: &mut Context, driver_ptr: *const TuiDriver) -> JsValue {
    let ptr = driver_ptr as usize;

    FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            let ref_id = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_escaped();

            // SAFETY: ptr was created from a valid reference that outlives this closure
            let driver = unsafe { &*(ptr as *const TuiDriver) };

            match driver.click(&ref_id) {
                Ok(()) => Ok(JsValue::undefined()),
                Err(e) => Err(boa_engine::JsError::from_opaque(JsValue::from(
                    JsString::from(format!("click error: {}", e).as_str()),
                ))),
            }
        }),
    )
    .name(js_string!("click"))
    .length(1)
    .build()
    .into()
}

/// Create the tui.doubleClick(ref) method for double-clicking by reference ID.
fn create_double_click_method(context: &mut Context, driver_ptr: *const TuiDriver) -> JsValue {
    let ptr = driver_ptr as usize;

    FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            let ref_id = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_escaped();

            // SAFETY: ptr was created from a valid reference that outlives this closure
            let driver = unsafe { &*(ptr as *const TuiDriver) };

            match driver.double_click(&ref_id) {
                Ok(()) => Ok(JsValue::undefined()),
                Err(e) => Err(boa_engine::JsError::from_opaque(JsValue::from(
                    JsString::from(format!("doubleClick error: {}", e).as_str()),
                ))),
            }
        }),
    )
    .name(js_string!("doubleClick"))
    .length(1)
    .build()
    .into()
}

/// Create the tui.rightClick(ref) method for right-clicking by reference ID.
fn create_right_click_method(context: &mut Context, driver_ptr: *const TuiDriver) -> JsValue {
    let ptr = driver_ptr as usize;

    FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            let ref_id = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_escaped();

            // SAFETY: ptr was created from a valid reference that outlives this closure
            let driver = unsafe { &*(ptr as *const TuiDriver) };

            match driver.right_click(&ref_id) {
                Ok(()) => Ok(JsValue::undefined()),
                Err(e) => Err(boa_engine::JsError::from_opaque(JsValue::from(
                    JsString::from(format!("rightClick error: {}", e).as_str()),
                ))),
            }
        }),
    )
    .name(js_string!("rightClick"))
    .length(1)
    .build()
    .into()
}

/// Create the tui.hover(ref) method for hovering over an element by reference ID.
fn create_hover_method(context: &mut Context, driver_ptr: *const TuiDriver) -> JsValue {
    let ptr = driver_ptr as usize;

    FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            let ref_id = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_escaped();

            // SAFETY: ptr was created from a valid reference that outlives this closure
            let driver = unsafe { &*(ptr as *const TuiDriver) };

            match driver.hover(&ref_id) {
                Ok(()) => Ok(JsValue::undefined()),
                Err(e) => Err(boa_engine::JsError::from_opaque(JsValue::from(
                    JsString::from(format!("hover error: {}", e).as_str()),
                ))),
            }
        }),
    )
    .name(js_string!("hover"))
    .length(1)
    .build()
    .into()
}

/// Create the tui.drag(startRef, endRef) method for dragging between elements.
fn create_drag_method(context: &mut Context, driver_ptr: *const TuiDriver) -> JsValue {
    let ptr = driver_ptr as usize;

    FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            let start_ref = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_escaped();
            let end_ref = args
                .get_or_undefined(1)
                .to_string(ctx)?
                .to_std_string_escaped();

            // SAFETY: ptr was created from a valid reference that outlives this closure
            let driver = unsafe { &*(ptr as *const TuiDriver) };

            match driver.drag(&start_ref, &end_ref) {
                Ok(()) => Ok(JsValue::undefined()),
                Err(e) => Err(boa_engine::JsError::from_opaque(JsValue::from(
                    JsString::from(format!("drag error: {}", e).as_str()),
                ))),
            }
        }),
    )
    .name(js_string!("drag"))
    .length(2)
    .build()
    .into()
}

/// Create the tui.waitForText(text, timeoutMs?) method.
/// Blocks until text appears on screen or timeout. Returns boolean.
fn create_wait_for_text_method(context: &mut Context, driver_ptr: *const TuiDriver) -> JsValue {
    let ptr = driver_ptr as usize;

    FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            let text = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_escaped();
            let timeout_ms = if args.get_or_undefined(1).is_undefined() {
                5000 // default timeout
            } else {
                args.get_or_undefined(1).to_u32(ctx)? as u64
            };

            // SAFETY: ptr was created from a valid reference that outlives script execution.
            // This runs inside block_in_place, so we use Handle::current().block_on()
            // to properly await the async driver method.
            let driver = unsafe { &*(ptr as *const TuiDriver) };
            let result = tokio::runtime::Handle::current()
                .block_on(driver.wait_for_text(&text, timeout_ms));

            match result {
                Ok(found) => Ok(JsValue::from(found)),
                Err(e) => Err(boa_engine::JsError::from_opaque(JsValue::from(
                    JsString::from(format!("waitForText error: {}", e).as_str()),
                ))),
            }
        }),
    )
    .name(js_string!("waitForText"))
    .length(1)
    .build()
    .into()
}

/// Create the tui.waitForIdle(timeoutMs?, idleMs?) method.
/// Blocks until screen stops changing or timeout. Returns boolean.
fn create_wait_for_idle_method(context: &mut Context, driver_ptr: *const TuiDriver) -> JsValue {
    let ptr = driver_ptr as usize;

    FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            let timeout_ms = if args.get_or_undefined(0).is_undefined() {
                5000 // default timeout
            } else {
                args.get_or_undefined(0).to_u32(ctx)? as u64
            };
            let idle_ms = if args.get_or_undefined(1).is_undefined() {
                100 // default idle time
            } else {
                args.get_or_undefined(1).to_u32(ctx)? as u64
            };

            // SAFETY: ptr was created from a valid reference that outlives script execution.
            // This runs inside block_in_place, so we use Handle::current().block_on()
            // to properly await the async driver method.
            let driver = unsafe { &*(ptr as *const TuiDriver) };
            let result = tokio::runtime::Handle::current()
                .block_on(driver.wait_for_idle(idle_ms, timeout_ms));

            match result {
                Ok(()) => Ok(JsValue::from(true)),
                Err(_) => Ok(JsValue::from(false)), // Timeout returns false
            }
        }),
    )
    .name(js_string!("waitForIdle"))
    .length(0)
    .build()
    .into()
}

/// Create the tui.resize(cols, rows) method for resizing the terminal.
fn create_resize_method(context: &mut Context, driver_ptr: *const TuiDriver) -> JsValue {
    let ptr = driver_ptr as usize;

    FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            let cols = args.get_or_undefined(0).to_u32(ctx)? as u16;
            let rows = args.get_or_undefined(1).to_u32(ctx)? as u16;

            // SAFETY: ptr was created from a valid reference that outlives this closure
            let driver = unsafe { &*(ptr as *const TuiDriver) };

            match driver.resize(cols, rows) {
                Ok(()) => Ok(JsValue::undefined()),
                Err(e) => Err(boa_engine::JsError::from_opaque(JsValue::from(
                    JsString::from(format!("resize error: {}", e).as_str()),
                ))),
            }
        }),
    )
    .name(js_string!("resize"))
    .length(2)
    .build()
    .into()
}

/// Create the tui.sendSignal(signal) method for sending signals to the process.
fn create_send_signal_method(context: &mut Context, driver_ptr: *const TuiDriver) -> JsValue {
    let ptr = driver_ptr as usize;

    FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            let signal_str = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_escaped();

            // Parse the signal
            let signal = Signal::parse(&signal_str).map_err(|e| {
                boa_engine::JsError::from_opaque(JsValue::from(JsString::from(e.as_str())))
            })?;

            // SAFETY: ptr was created from a valid reference that outlives this closure
            let driver = unsafe { &*(ptr as *const TuiDriver) };

            match driver.send_signal(signal) {
                Ok(()) => Ok(JsValue::undefined()),
                Err(e) => Err(boa_engine::JsError::from_opaque(JsValue::from(
                    JsString::from(format!("sendSignal error: {}", e).as_str()),
                ))),
            }
        }),
    )
    .name(js_string!("sendSignal"))
    .length(1)
    .build()
    .into()
}

/// Create the tui.getScrollback() method for getting scrollback line count.
fn create_get_scrollback_method(context: &mut Context, driver_ptr: *const TuiDriver) -> JsValue {
    let ptr = driver_ptr as usize;

    FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_copy_closure(move |_this, _args, _ctx| {
            // SAFETY: ptr was created from a valid reference that outlives this closure
            let driver = unsafe { &*(ptr as *const TuiDriver) };
            let scrollback = driver.get_scrollback();
            Ok(JsValue::from(scrollback as i32))
        }),
    )
    .name(js_string!("getScrollback"))
    .length(0)
    .build()
    .into()
}

/// Create the tui.getInput(chars?) method for getting raw input buffer.
fn create_get_input_method(context: &mut Context, driver_ptr: *const TuiDriver) -> JsValue {
    let ptr = driver_ptr as usize;

    FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            let chars = if args.get_or_undefined(0).is_undefined() {
                10000 // default
            } else {
                args.get_or_undefined(0).to_u32(ctx)? as usize
            };

            // SAFETY: ptr was created from a valid reference that outlives this closure
            let driver = unsafe { &*(ptr as *const TuiDriver) };
            let input = driver.get_input_buffer(chars);
            Ok(JsValue::from(JsString::from(input.as_str())))
        }),
    )
    .name(js_string!("getInput"))
    .length(0)
    .build()
    .into()
}

/// Create the tui.getOutput(chars?) method for getting raw output buffer.
fn create_get_output_method(context: &mut Context, driver_ptr: *const TuiDriver) -> JsValue {
    let ptr = driver_ptr as usize;

    FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            let chars = if args.get_or_undefined(0).is_undefined() {
                10000 // default
            } else {
                args.get_or_undefined(0).to_u32(ctx)? as usize
            };

            // SAFETY: ptr was created from a valid reference that outlives this closure
            let driver = unsafe { &*(ptr as *const TuiDriver) };
            let output = driver.get_output_buffer(chars);
            Ok(JsValue::from(JsString::from(output.as_str())))
        }),
    )
    .name(js_string!("getOutput"))
    .length(0)
    .build()
    .into()
}

#[cfg(test)]
mod tests {
    // Note: Full integration tests require a running TUI session.
    // Unit tests for basic JavaScript execution would require mocking TuiDriver.
}
