//! Shared meanings for compact property rows and numeric controls. Detailed
//! component-specific descriptions at their call sites take precedence.
pub fn control_description(label: &str) -> &'static str {
    match label {
        "Name" => "Name this item so it is easy to recognize in the scene and source menus.",
        "Color" => {
            "Choose this control’s display color. Auto follows its modulation source or the app accent; it does not change the visualization’s palette."
        }
        "Parameter" => {
            "The component parameter controlled by this scene macro. Drag a different inspector parameter onto the macro to replace it while retaining its modulation connection."
        }
        "Component" => {
            "The component that owns the assigned parameter. Scene macros keep this target when you select another component."
        }
        "Manual value" => {
            "The parameter’s base value before modulation. Drag or type an exact value; the source adds movement around this base."
        }
        "Live value" => {
            "The current parameter value after enabled macro modulation and safety limits. The manual base remains unchanged."
        }
        "Knob range" => {
            "The parameter values represented by the knob’s 0–100% travel. Modulation strength is measured relative to this span."
        }
        "Default" => "The parameter’s original value, restored by Reset value.",
        "Source" => {
            "Choose a modulation source created in Signals. Each macro has one source; a source can drive many macros. None disconnects modulation without changing the manual value."
        }
        "Input" => {
            "Choose what feeds this channel. Main audio follows the capture device in Settings. An LFO generates movement without audio. MIDI receives controllers or notes from inputs enabled in Settings → MIDI. OSC and direct Ableton integration are reserved for future support."
        }
        "Detector" => {
            "Choose how to turn audio into a control signal. Band energy follows a frequency range; hit detectors emphasize transients; intensity follows the mix. These are estimates, not isolated instrument stems."
        }
        "Output" => {
            "The signal’s output type: unipolar levels, signed bipolar values, or transient events with an envelope. Influence and range processing can change its final amplitude."
        }
        "Enabled" => {
            "Enable or bypass this macro’s modulation. Bypass keeps its source and strength for later use; the manual parameter value still applies."
        }
        "Strength" => {
            "How strongly the signal moves this parameter, as a percentage of the knob range. Negative values reverse movement. This is also adjustable by dragging the knob’s outer ring."
        }
        "Invert" => {
            "Negate the incoming signal before applying its range and strength. A positive unipolar source then moves in the opposite direction; this is not a 1-minus-input operation."
        }
        "Range start" | "Output min" | "Range minimum" => {
            "The offset applied when a connected source is zero. With a 0–1 source, this is the lower mapped value before strength. Disconnected sources fade the offset to zero too."
        }
        "Range end" | "Output max" | "Range maximum" => {
            "The mapped value when a source is one, before strength. The difference between range end and start scales the source; bipolar sources can extend below the start."
        }
        "Signal output" => {
            "The live value leaving the selected source, including its envelope, normalization, influence, smoothing, mute and solo state."
        }
        "Status" => {
            "Whether this source is live, waiting, unavailable, or muted. Unavailable input fades its influence to zero and resumes when it returns."
        }
        "Gain dB" => {
            "Raise or lower the level entering detection. Increase for quiet audio; lower it if the detector stays near its maximum. This does not change speaker volume."
        }
        "Noise floor dB" | "Floor dB" => {
            "Treat audio below this level as silence. Raise the floor to reject background noise; lower it to include quiet material."
        }
        "Sensitivity curve" | "Curve" => {
            "Shape how detected energy becomes control output. Adjust this when the signal spends too much time near one end of its range; watch the output meter while changing it."
        }
        "Adaptive" => {
            "Adapt sensitivity to recent audio levels so quieter and louder passages can both produce useful motion. Turn off for a more consistent absolute level response."
        }
        "Threshold" | "Trigger threshold" => {
            "The level needed to trigger an envelope. Lower it for more frequent reactions; raise it to emphasize stronger hits."
        }
        "Retrigger ms" | "Hold ms" => {
            "Minimum interval between triggers. Increase it to suppress repeated triggers within a single hit; decrease it for faster rhythmic material."
        }
        "Response" => {
            "Choose the response shape: follow the incoming level smoothly or trigger an ADSR envelope. Attack and release control rise and fall; ADSR adds decay and sustain."
        }
        "Sustain" => {
            "The level retained after the decay stage while a gated envelope remains active. This is a level, not a duration."
        }
        "Input low" => {
            "The lower detector/envelope value that normalization maps to zero. Set this near the bottom of a narrow observed range, such as 0.50."
        }
        "Input high" => {
            "The upper detector/envelope value that normalization maps to one. For a 0.50–0.55 signal, low 0.50 and high 0.55 expand it to 0–1. Values outside the range are clamped."
        }
        "Smoothing ms" => {
            "Smooth the final channel output after normalization and influence. Zero is immediate; longer times reduce jitter but make movement respond more slowly."
        }
        "Rate Hz" => {
            "LFO cycles per second. 1 Hz completes one cycle each second; 0.25 Hz takes four seconds. The oscillator runs independently of audio and preserves phase when its rate changes."
        }
        "Phase" => {
            "Offset the LFO within its cycle: 0 is the beginning, 0.5 is halfway around, and 1 is a full cycle."
        }
        "Waveform" => {
            "Choose the LFO motion shape. Sine is smooth, triangle moves at a constant rate between extremes, ramps sweep in one direction, and square alternates abruptly."
        }
        "Ambient motion" => {
            "Movement that remains when audio modulation is quiet or absent. Lower it for stillness between hits; raise it for continuous motion."
        }
        "Modulation amount" => {
            "Overall intensity of this component’s built-in audio mappings. Individual scene macros have their own modulation strength."
        }
        "Palette" => {
            "Choose a visualization gradient or saved color preset. These colors belong to this component and are independent of the application GUI theme."
        }
        "Trace" => {
            "Choose how this component draws the signal. The active style determines which appearance controls are available; analysis is unchanged."
        }
        "Interface zoom" => {
            "Scale all application controls and text. This is a local GUI preference and does not change camera zoom or saved scene geometry."
        }
        "FFT size" => {
            "Samples per frequency analysis window. Larger values separate nearby frequencies more clearly but take longer to collect; smaller values react faster with less frequency detail."
        }
        "Window" => {
            "Choose the analysis window used before the FFT. This controls the tradeoff between frequency leakage and peak width."
        }
        "Analysis rate" => {
            "How often frequency analysis updates. Higher rates give more frequent snapshots and use more CPU; the display frame rate is separate."
        }
        "Capture source" => {
            "Select the system output or input device analyzed by Main audio channels. Device preferences are local to this machine."
        }
        "Capture status" => {
            "The state of the audio connection. If capture is unavailable, check the selected device and operating-system audio-recording permission."
        }
        "Format" => "Sample rate and channel count reported by the active audio capture device.",
        "Captured signal" => {
            "RMS level of captured audio in dBFS. Zero dBFS is digital full scale; quieter audio has negative values."
        }
        "Component inspector" => {
            "Show or hide the right inspector. Clicking a component, signal, or macro chooses which details it displays."
        }
        "Info View / hover help" => {
            "Show contextual help at the bottom left. Its height follows the open Signals or Macros dock. F1 also toggles it."
        }
        "Application fullscreen" => {
            "Expand the application into fullscreen. Individual component panes can also be expanded using their corner control."
        }
        _ => "",
    }
}
