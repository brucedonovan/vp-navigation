//! JavaScript bindings. The methods mirror [`vp_navigation::Navigator`];
//! frames and state cross the boundary as JSON so any renderer can parse them
//! without sharing Rust types.

#[cfg(target_arch = "wasm32")]
mod bind {
    use wasm_bindgen::prelude::*;

    use vp_navigation::Navigator;

    #[wasm_bindgen]
    pub struct VpNavigator {
        inner: Navigator,
    }

    #[wasm_bindgen]
    impl VpNavigator {
        #[wasm_bindgen(constructor)]
        pub fn new(config_json: &str) -> Result<VpNavigator, JsValue> {
            Navigator::from_config_json(config_json)
                .map(|inner| VpNavigator { inner })
                .map_err(|err| JsValue::from_str(&err.to_string()))
        }

        pub fn tick(&mut self, dt_ms: f64) {
            self.inner.tick(dt_ms);
        }

        #[wasm_bindgen(js_name = moveBy)]
        pub fn move_by(&mut self, delta: f64) {
            self.inner.move_by(delta);
        }

        pub fn step(&mut self, steps: i32) {
            self.inner.step(steps);
        }

        pub fn focus(&mut self, node_id: &str) -> Result<(), JsValue> {
            self.inner
                .focus(node_id)
                .map_err(|err| JsValue::from_str(&err.to_string()))
        }

        pub fn activate(&mut self, node_id: &str) -> Result<(), JsValue> {
            self.inner
                .activate(node_id)
                .map_err(|err| JsValue::from_str(&err.to_string()))
        }

        #[wasm_bindgen(js_name = takeBranch)]
        pub fn take_branch(&mut self, branch_id: &str) -> Result<(), JsValue> {
            self.inner
                .take_branch(branch_id)
                .map_err(|err| JsValue::from_str(&err.to_string()))
        }

        #[wasm_bindgen(js_name = returnToParent)]
        pub fn return_to_parent(&mut self) -> Result<(), JsValue> {
            self.inner
                .return_to_parent()
                .map_err(|err| JsValue::from_str(&err.to_string()))
        }

        #[wasm_bindgen(js_name = setReducedMotion)]
        pub fn set_reduced_motion(&mut self, enabled: bool) {
            self.inner.set_reduced_motion(enabled);
        }

        #[wasm_bindgen(js_name = stateJson)]
        pub fn state_json(&self) -> String {
            self.inner.state_json()
        }

        #[wasm_bindgen(js_name = frameJson)]
        pub fn frame_json(&self) -> String {
            self.inner.frame_json()
        }
    }
}
