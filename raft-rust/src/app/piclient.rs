//! Simple raspberry pi sense hat server client

use std::net::UdpSocket;
use std::net::SocketAddr;

pub const STRIPE_PIXEL: (u8, u8, u8) = (64, 64, 64);
pub const OFF_PIXEL: (u8, u8, u8) = (0, 0, 0);

pub struct PiClient {
    id: u8,
    coords: (u8, u8),
    sense_hat: SenseHat,
}
impl PiClient {
    pub fn new(id: u8, coords: (u8, u8), sense_hat: SenseHat) -> Self {
        Self { id, coords, sense_hat, }
    }

    pub fn stripe(&self) {
        for i in 0..8 {
            if i < self.id {
                self.sense_hat.pixel((i, 0), STRIPE_PIXEL);
            } else {
                self.sense_hat.pixel((i, 0), OFF_PIXEL);
            }
        }
    }

    pub fn light(&self, rgb: (u8, u8, u8)) {
        self.sense_hat.pixel(self.coords, rgb);
    }
}

// (my_addr, remote_addr)
#[derive(Copy, Clone)]
pub struct SenseHat(pub SocketAddr);
impl SenseHat {
    pub fn pixel(&self, (x, y): (u8, u8), (r, g, b): (u8, u8, u8)) {
        let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
        let string = format!("pixel {},{},{},{},{}", x, y, r, g, b);
        match socket.send_to(string.as_bytes(), &self.0) {
            Ok(_) => {},
            Err(_) => println!("error setting pixel {:?} at {:?}", (r, g, b), (x, y)),
        }
    }
}

#[cfg(test)]
pub mod test {
    use super::*;
    use std::net::SocketAddr;
    use std::thread;
    use std::time::Duration;

    #[test]
    pub fn stripe() {
        let pi_addr: SocketAddr = "10.0.1.2:12345".parse().unwrap();
        let sense_hat = SenseHat(pi_addr);
        let client1 = PiClient::new(1, (4, 4), sense_hat.clone());
        let client2 = PiClient::new(3, (5, 5), sense_hat.clone());
        client1.light((55, 55, 0));
        client2.light((0, 55, 200));
        client1.stripe();
        thread::sleep(Duration::from_millis(5000));
        client2.stripe();
    }
}
