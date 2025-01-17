/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;

use dom_struct::dom_struct;
use js::typedarray::{Float64, Float64Array};

use super::bindings::typedarrays::HeapTypedArray;
use crate::dom::bindings::codegen::Bindings::GamepadBinding::{GamepadHand, GamepadMethods};
use crate::dom::bindings::codegen::Bindings::GamepadButtonListBinding::GamepadButtonListMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::num::Finite;
use crate::dom::bindings::reflector::{reflect_dom_object_with_proto, DomObject, Reflector};
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::bindings::str::DOMString;
use crate::dom::event::Event;
use crate::dom::eventtarget::EventTarget;
use crate::dom::gamepadbuttonlist::GamepadButtonList;
use crate::dom::gamepadevent::{GamepadEvent, GamepadEventType};
use crate::dom::gamepadpose::GamepadPose;
use crate::dom::globalscope::GlobalScope;
use crate::script_runtime::JSContext;

// This value is for determining when to consider a non-digital button "pressed".
// Like Gecko and Chromium it derives from the XInput trigger threshold.
const BUTTON_PRESS_THRESHOLD: f64 = 30.0 / 255.0;

#[dom_struct]
pub struct Gamepad {
    reflector_: Reflector,
    gamepad_id: u32,
    id: String,
    index: Cell<i32>,
    connected: Cell<bool>,
    timestamp: Cell<f64>,
    mapping_type: String,
    #[ignore_malloc_size_of = "mozjs"]
    axes: HeapTypedArray<Float64>,
    buttons: Dom<GamepadButtonList>,
    pose: Option<Dom<GamepadPose>>,
    #[ignore_malloc_size_of = "Defined in rust-webvr"]
    hand: GamepadHand,
    axis_bounds: (f64, f64),
    button_bounds: (f64, f64),
}

impl Gamepad {
    fn new_inherited(
        gamepad_id: u32,
        id: String,
        index: i32,
        connected: bool,
        timestamp: f64,
        mapping_type: String,
        buttons: &GamepadButtonList,
        pose: Option<&GamepadPose>,
        hand: GamepadHand,
        axis_bounds: (f64, f64),
        button_bounds: (f64, f64),
    ) -> Gamepad {
        Self {
            reflector_: Reflector::new(),
            gamepad_id: gamepad_id,
            id: id,
            index: Cell::new(index),
            connected: Cell::new(connected),
            timestamp: Cell::new(timestamp),
            mapping_type: mapping_type,
            axes: HeapTypedArray::default(),
            buttons: Dom::from_ref(buttons),
            pose: pose.map(Dom::from_ref),
            hand: hand,
            axis_bounds: axis_bounds,
            button_bounds: button_bounds,
        }
    }

    pub fn new(
        global: &GlobalScope,
        gamepad_id: u32,
        id: String,
        axis_bounds: (f64, f64),
        button_bounds: (f64, f64),
    ) -> DomRoot<Gamepad> {
        Self::new_with_proto(global, gamepad_id, id, axis_bounds, button_bounds)
    }

    /// When we construct a new gamepad, we initialize the number of buttons and
    /// axes corresponding to the "standard" gamepad mapping.
    /// The spec says UAs *may* do this for fingerprint mitigation, and it also
    /// happens to simplify implementation
    /// <https://www.w3.org/TR/gamepad/#fingerprinting-mitigation>
    fn new_with_proto(
        global: &GlobalScope,
        gamepad_id: u32,
        id: String,
        axis_bounds: (f64, f64),
        button_bounds: (f64, f64),
    ) -> DomRoot<Gamepad> {
        let button_list = GamepadButtonList::init_buttons(global);
        let gamepad = reflect_dom_object_with_proto(
            Box::new(Gamepad::new_inherited(
                gamepad_id,
                id,
                0,
                false,
                0.,
                String::from("standard"),
                &button_list,
                None,
                GamepadHand::_empty,
                axis_bounds,
                button_bounds,
            )),
            global,
            None,
        );
        gamepad.init_axes();
        gamepad
    }
}

impl GamepadMethods for Gamepad {
    // https://w3c.github.io/gamepad/#dom-gamepad-id
    fn Id(&self) -> DOMString {
        DOMString::from(self.id.clone())
    }

    // https://w3c.github.io/gamepad/#dom-gamepad-index
    fn Index(&self) -> i32 {
        self.index.get()
    }

    // https://w3c.github.io/gamepad/#dom-gamepad-connected
    fn Connected(&self) -> bool {
        self.connected.get()
    }

    // https://w3c.github.io/gamepad/#dom-gamepad-timestamp
    fn Timestamp(&self) -> Finite<f64> {
        Finite::wrap(self.timestamp.get())
    }

    // https://w3c.github.io/gamepad/#dom-gamepad-mapping
    fn Mapping(&self) -> DOMString {
        DOMString::from(self.mapping_type.clone())
    }

    // https://w3c.github.io/gamepad/#dom-gamepad-axes
    fn Axes(&self, _cx: JSContext) -> Float64Array {
        self.axes
            .get_internal()
            .expect("Failed to get gamepad axes.")
    }

    // https://w3c.github.io/gamepad/#dom-gamepad-buttons
    fn Buttons(&self) -> DomRoot<GamepadButtonList> {
        DomRoot::from_ref(&*self.buttons)
    }

    // https://w3c.github.io/gamepad/extensions.html#gamepadhand-enum
    fn Hand(&self) -> GamepadHand {
        self.hand
    }

    // https://w3c.github.io/gamepad/extensions.html#dom-gamepad-pose
    fn GetPose(&self) -> Option<DomRoot<GamepadPose>> {
        self.pose.as_ref().map(|p| DomRoot::from_ref(&**p))
    }
}

#[allow(dead_code)]
impl Gamepad {
    pub fn gamepad_id(&self) -> u32 {
        self.gamepad_id
    }

    pub fn update_connected(&self, connected: bool) {
        if self.connected.get() == connected {
            return;
        }
        self.connected.set(connected);

        let event_type = if connected {
            GamepadEventType::Connected
        } else {
            GamepadEventType::Disconnected
        };

        self.notify_event(event_type);
    }

    pub fn update_index(&self, index: i32) {
        self.index.set(index);
    }

    pub fn update_timestamp(&self, timestamp: f64) {
        self.timestamp.set(timestamp);
    }

    pub fn notify_event(&self, event_type: GamepadEventType) {
        let event = GamepadEvent::new_with_type(&self.global(), event_type, &self);
        event
            .upcast::<Event>()
            .fire(self.global().as_window().upcast::<EventTarget>());
    }

    /// Initialize the number of axes in the "standard" gamepad mapping.
    /// <https://www.w3.org/TR/gamepad/#dfn-initializing-axes>
    fn init_axes(&self) {
        let initial_axes: Vec<f64> = vec![
            0., // Horizontal axis for left stick (negative left/positive right)
            0., // Vertical axis for left stick (negative up/positive down)
            0., // Horizontal axis for right stick (negative left/positive right)
            0., // Vertical axis for right stick (negative up/positive down)
        ];
        self.axes
            .set_data(GlobalScope::get_cx(), &initial_axes)
            .expect("Failed to set axes data on gamepad.")
    }

    #[allow(unsafe_code)]
    /// <https://www.w3.org/TR/gamepad/#dfn-map-and-normalize-axes>
    pub fn map_and_normalize_axes(&self, axis_index: usize, value: f64) {
        // Let normalizedValue be 2 (logicalValue − logicalMinimum) / (logicalMaximum − logicalMinimum) − 1.
        let numerator = value - self.axis_bounds.0;
        let denominator = self.axis_bounds.1 - self.axis_bounds.0;
        if denominator != 0.0 && denominator.is_finite() {
            let normalized_value: f64 = 2.0 * numerator / denominator - 1.0;
            if normalized_value.is_finite() {
                let mut axis_vec = self
                    .axes
                    .internal_to_option()
                    .expect("Axes have not been initialized!");
                unsafe {
                    axis_vec.as_mut_slice()[axis_index] = normalized_value;
                }
            } else {
                warn!("Axis value is not finite!");
            }
        } else {
            warn!("Axis bounds difference is either 0 or non-finite!");
        }
    }

    /// <https://www.w3.org/TR/gamepad/#dfn-map-and-normalize-buttons>
    pub fn map_and_normalize_buttons(&self, button_index: usize, value: f64) {
        // Let normalizedValue be (logicalValue − logicalMinimum) / (logicalMaximum − logicalMinimum).
        let numerator = value - self.button_bounds.0;
        let denominator = self.button_bounds.1 - self.button_bounds.0;
        if denominator != 0.0 && denominator.is_finite() {
            let normalized_value: f64 = numerator / denominator;
            if normalized_value.is_finite() {
                let pressed = normalized_value >= BUTTON_PRESS_THRESHOLD;
                // TODO: Determine a way of getting touch capability for button
                if let Some(button) = self.buttons.IndexedGetter(button_index as u32) {
                    button.update(pressed, /*touched*/ pressed, normalized_value);
                }
            } else {
                warn!("Button value is not finite!");
            }
        } else {
            warn!("Button bounds difference is either 0 or non-finite!");
        }
    }
}
