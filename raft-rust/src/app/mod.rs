pub mod piclient;

use crate::raft::app::ApplicationStateMachine;
use crate::app::piclient::PiClient;

/// Actual business logic.  This is what all the raft machinery is about: setting a color variable.

#[derive(Copy, Clone, Eq, PartialEq, Default, Debug, Serialize, Deserialize)]
pub struct Color(u8, u8, u8);
impl Color {
    pub fn new(red: u8, green: u8, blue: u8) -> Color {
        Color(red, green, blue)
    }

    pub fn unwrap(&self) -> (u8, u8, u8) {
        (self.0, self.1, self.2)
    }

    pub fn parse(input: String) -> Self {
        let parts = input.split(",").collect::<Vec<&str>>();
        Color(parts[0].trim().parse().unwrap(), parts[1].trim().parse().unwrap(), parts[2].trim().parse().unwrap())
    }
}

pub struct ColorStateMachine {
    state: Color,
    piclient: PiClient,
}
impl ColorStateMachine {
    pub fn new(piclient: PiClient) -> Self {
        Self {
            state: Color::default(),
            piclient,
        }
    }
}
impl ApplicationStateMachine<Color> for ColorStateMachine {
    type Snapshot = Color;
    type Output = ();

    fn apply(&mut self, transition: Color) {
        self.state = transition;
        self.piclient.light(self.state.unwrap());
    }

    fn snapshot(&mut self) -> Self::Snapshot {
        self.state
    }

    fn i_became_leader(&self) {
        self.piclient.stripe();
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    pub fn test_parse() {
        let color = Color::new(123, 200, 6);
        let color1 = Color::parse("123, 200, 6".to_string());
        let color2 = Color::parse("123,200,6".to_string());
        assert_eq!(color, color1);
        assert_eq!(color, color2);
    }

    #[test]
    pub fn serde_test() {
        println!("{}", serde_json::to_string(&Color::new(123, 200, 6)).unwrap());
    }
}
