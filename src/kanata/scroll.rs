use super::*;

pub struct ScrollState {
    pub direction: MWheelDirection,
    pub interval: u16,
    pub ticks_until_scroll: u16,
    pub distance: u16,
    pub scroll_accel_state: Option<ScrollAccelState>,
}

pub struct ScrollAccelState {
    pub deceleration_multiplier: f32,
    pub acceleration_multiplier: f32,
    pub max_velocity: f32,
    pub current_velocity: f32,
    pub scroll_released: bool,
}

pub(crate) fn update_scrollstate_get_result(
    state: &mut Option<ScrollState>,
) -> Option<(MWheelDirection, u16)> {
    let Some(state) = state else {
        return None;
    };
    if state.ticks_until_scroll == 0 {
        state.ticks_until_scroll = state.interval - 1;
        let direction = state.direction;
        let distance = state.distance;

        Some(match &mut state.scroll_accel_state {
            Some(acs) => match acs.scroll_released {
                false => {
                    let new_velocity = f32::min(
                        acs.max_velocity,
                        acs.current_velocity * acs.acceleration_multiplier,
                    );
                    acs.current_velocity = new_velocity;
                    (direction, new_velocity as u16)
                }
                true => {
                    let mut new_velocity = acs.current_velocity * acs.deceleration_multiplier;
                    if new_velocity < 5.0 {
                        new_velocity = 0.0;
                    }
                    acs.current_velocity = new_velocity;
                    (direction, new_velocity as u16)
                }
            },
            None => (direction, distance),
        })
    } else {
        state.ticks_until_scroll -= 1;
        None
    }
}
