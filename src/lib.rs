use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use crate::replies::Reply;


pub mod connect {
    use super::*;

    pub trait Manager {
        fn address(&self) -> &str;
    }

    pub struct TcpManager {
        address: String,
    }

    impl TcpManager {
        pub fn new(address: String) -> Self {
            TcpManager { address }
        }
    }

    impl Manager for TcpManager {
        fn address(&self) -> &str {
            &self.address
        }
    }

    pub struct Reader {
        stream: TcpStream,
    }
    
    impl Reader {
        pub fn new(stream: TcpStream) -> Self {
            Reader { stream }
        }
    
        pub fn read_message(&mut self) -> Result<super::Message, Box<dyn Error>> {
            let mut len_buf = [0; 4];
            self.stream.read_exact(&mut len_buf)?;
            let len = u32::from_be_bytes(len_buf);
    
            let mut msg_buf = vec![0; len as usize];
            self.stream.read_exact(&mut msg_buf)?;
    
            let msg: super::Message = serde_json::from_slice(&msg_buf)?;
            Ok(msg)
        }
    }
    
    pub struct Writer {
        stream: TcpStream,
    }
    
    impl Writer {
        pub fn new(stream: TcpStream) -> Self {
            Writer { stream }
        }
    
        pub fn write_message(&mut self, reply: super::Reply) -> Result<(), Box<dyn Error>> {
            let msg = super::Message::Reply(reply);
            let msg_json = serde_json::to_string(&msg)?;
    
            let len_bytes = (msg_json.len() as u32).to_be_bytes();
            self.stream.write_all(&len_bytes)?;
            self.stream.write_all(msg_json.as_bytes())?;
            Ok(())
        }
    }
}

pub mod replies {
    use super::*;

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    pub enum Reply {
        Ok,
        Value(CellValue),
        Error(String),
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Message {
    Command(String),
    Reply(replies::Reply),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CellValue {
    Number(f64),
    Text(String),
    Error(String),
}

pub struct RSheet {
    cells: Arc<Mutex<HashMap<String, CellValue>>>,
}

impl RSheet {
    pub fn new() -> Self {
        println!("Initializing RSheet with an empty hashmap.");
        RSheet {
            cells: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn handle_command(&self, command: String) -> replies::Reply {
        let parts: Vec<&str> = command.split_whitespace().collect();
        match parts[0] {
            "set" if parts.len() == 3 => {
                let cell = parts[1];
                let value = parts[2];
                // Check if value is just a number or an expression
                if value.parse::<f64>().is_ok() {
                    self.set_cell(cell, value.to_string())
                } else {
                    // It's an expression
                    let expr = parts[2..].join(" ");
                    self.set_cell(cell, expr)
                }
            },
            "get" if parts.len() == 2 => self.get_cell(parts[1]),
            _ => replies::Reply::Error("Invalid command format".to_string()),
        }
    }

    fn set_cell_direct(&self, cell: &str, value: &str) -> replies::Reply {
        match value.parse::<f64>() {
            Ok(num) => {
                self.cells.lock().unwrap().insert(cell.to_string(), CellValue::Number(num));
                replies::Reply::Ok
            },
            Err(_) => replies::Reply::Error("Invalid numeric value".to_string())
        }
    }

    fn get_cell(&self, cell: &str) -> replies::Reply {
        println!("Getting value for cell: {}", cell);
        match self.cells.lock().unwrap().get(cell) {
            Some(value) => {
                println!("Found value: {:?}", value);
                replies::Reply::Value(value.clone())
            },
            None => {
                println!("No value found for cell: {}", cell);
                replies::Reply::Value(CellValue::Error(format!("Cell {} not found", cell)))
            },
        }
    }

    fn set_cell(&self, cell: &str, expr: String) -> replies::Reply {
        println!("Setting cell: {} with expr: {}", cell, expr);
        let runner = CommandRunner::new(self.cells.clone());
        let result = runner.run(&expr);
        match result {
            CellValue::Error(e) => {
                println!("Error in expression: {}", e);
                replies::Reply::Error(e)
            },
            value => {
                println!("Updating cell: {} with value: {:?}", cell, value);
                self.cells.lock().unwrap().insert(cell.to_string(), value);
                replies::Reply::Ok
            }
        }
    }
}

struct CommandRunner {
    values: Arc<Mutex<HashMap<String, CellValue>>>,
}

impl CommandRunner {
    fn new(values: Arc<Mutex<HashMap<String, CellValue>>>) -> Self {
        CommandRunner { values }
    }

    pub fn run(&self, expr: &str) -> CellValue {
        if let Ok(num) = expr.parse::<f64>() {
            return CellValue::Number(num);
        }
        let re = Regex::new(r"(\w+)\s*([\+\-\*\/])\s*(\w+)").unwrap();
        if let Some(caps) = re.captures(expr) {
            let left = self.eval_operand(caps.get(1).unwrap().as_str());
            let operator = caps.get(2).unwrap().as_str();
            let right = self.eval_operand(caps.get(3).unwrap().as_str());

            match operator {
                "+" => self.add(left, right),
                "-" => self.sub(left, right),
                "*" => self.mul(left, right),
                "/" => self.div(left, right),
                _ => CellValue::Error("Invalid operator".to_string()),
            }
        } else {
    
            CellValue::Error("Unsupported expression format: ".to_string() + expr)
        }
    }

    fn eval_operand(&self, operand: &str) -> CellValue {
        let values = self.values.lock().unwrap();
        match values.get(operand) {
            Some(val) => val.clone(),
            None => operand.parse::<f64>().map_or(
                CellValue::Error(format!("Invalid operand: {}", operand)),
                CellValue::Number
            )
        }
    }

    fn eval_expr<'a, I>(&self, tokens: &mut I) -> CellValue
    where
        I: Iterator<Item = &'a str>,
    {
        let mut result = self.eval_term(tokens);

        while let Some(op) = tokens.next() {
            let rhs = self.eval_term(tokens);
            result = match op {
                "+" => self.add(result, rhs),
                "-" => self.sub(result, rhs),
                _ => CellValue::Error(format!("Invalid operator: {}", op)),
            };
        }

        result
    }

    fn eval_term<'a, I>(&self, tokens: &mut I) -> CellValue
    where
        I: Iterator<Item = &'a str>,
    {
        let mut result = self.eval_factor(tokens);

        while let Some(op) = tokens.next() {
            let rhs = self.eval_factor(tokens);
            result = match op {
                "*" => self.mul(result, rhs),
                "/" => self.div(result, rhs),
                _ => {
                    tokens.next();
                    return result;
                }
            };
        }

        result
    }

    fn eval_factor<'a, I>(&self, tokens: &mut I) -> CellValue
    where
        I: Iterator<Item = &'a str>,
    {
        if let Some(token) = tokens.next() {
            if let Ok(value) = token.parse::<f64>() {
                CellValue::Number(value)
            } else if let Some(value) = self.values.lock().unwrap().get(token) {
                value.clone()
            } else {
                CellValue::Error(format!("Invalid reference: {}", token))
            }
        } else {
            CellValue::Error("Unexpected end of expression".to_string())
        }
    }
    fn add(&self, lhs: CellValue, rhs: CellValue) -> CellValue {
        match (lhs, rhs) {
            (CellValue::Number(lhs), CellValue::Number(rhs)) => CellValue::Number(lhs + rhs),
            _ => CellValue::Error("Invalid operands for addition".to_string()),
        }
    }

    fn sub(&self, lhs: CellValue, rhs: CellValue) -> CellValue {
        match (lhs, rhs) {
            (CellValue::Number(lhs), CellValue::Number(rhs)) => CellValue::Number(lhs - rhs),
            _ => CellValue::Error("Invalid operands for subtraction".to_string()),
        }
    }

    fn mul(&self, lhs: CellValue, rhs: CellValue) -> CellValue {
        match (lhs, rhs) {
            (CellValue::Number(lhs), CellValue::Number(rhs)) => CellValue::Number(lhs * rhs),
            _ => CellValue::Error("Invalid operands for multiplication".to_string()),
        }
    }
    fn div(&self, lhs: CellValue, rhs: CellValue) -> CellValue {
        match (lhs, rhs) {
            (CellValue::Number(_), CellValue::Number(0.0)) => {
                CellValue::Error("Division by zero".to_string())
            }
            (CellValue::Number(lhs), CellValue::Number(rhs)) => CellValue::Number(lhs / rhs),
            _ => CellValue::Error("Invalid operands for division".to_string()),
        }
    }
}



pub fn start_server<M>(rsheet: Arc<RSheet>, manager: M) -> Result<(), Box<dyn Error>>
where
    M: connect::Manager + Sync,
{
    let address = manager.address().clone();
    let listener = std::net::TcpListener::bind(&address)?;
    loop {
        let (socket, _) = listener.accept()?;
        let rsheet = Arc::clone(&rsheet);

        std::thread::spawn(move || {
            let reader = socket.try_clone().expect("Failed to clone socket");
            let writer = socket;
            let mut reader = connect::Reader::new(reader);
            let mut writer = connect::Writer::new(writer);

            loop {
                match reader.read_message() {
                    Ok(Message::Command(cmd)) => {
                        let reply = futures::executor::block_on(rsheet.handle_command(cmd));
                        writer.write_message(reply).unwrap();
                    }
                    _ => break,
                }
            }
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rsheet() {
        let rsheet = RSheet::new();

        let reply = rsheet.handle_command("set A1 1".to_string()).await;
        assert_eq!(reply, replies::Reply::Ok);

        let reply = rsheet.handle_command("set B1 2".to_string()).await;
        assert_eq!(reply, replies::Reply::Ok);

        let reply = rsheet.handle_command("set C1 A1+B1".to_string()).await;
        assert_eq!(reply, replies::Reply::Ok);

        let reply = rsheet.handle_command("get A1".to_string()).await;
        assert_eq!(reply, replies::Reply::Value(CellValue::Number(1.0)));

        let reply = rsheet.handle_command("get B1".to_string()).await;
        assert_eq!(reply, replies::Reply::Value(CellValue::Number(2.0)));

        let reply = rsheet.handle_command("get C1".to_string()).await;
        assert_eq!(reply, replies::Reply::Value(CellValue::Number(3.0)));

        let reply = rsheet.handle_command("set D1 A1*B1".to_string()).await;
        assert_eq!(reply, replies::Reply::Ok);

        let reply = rsheet.handle_command("get D1".to_string()).await;
        assert_eq!(reply, replies::Reply::Value(CellValue::Number(2.0)));

        let reply = rsheet.handle_command("set E1 D1/C1".to_string()).await;
        assert_eq!(reply, replies::Reply::Ok);

        let reply = rsheet.handle_command("get E1".to_string()).await;
        assert_eq!(reply, replies::Reply::Value(CellValue::Number(2.0 / 3.0)));

        let reply = rsheet.handle_command("set F1 C1-A1".to_string()).await;
        assert_eq!(reply, replies::Reply::Ok);

        let reply = rsheet.handle_command("get F1".to_string()).await;
        assert_eq!(reply, replies::Reply::Value(CellValue::Number(2.0)));

        let reply = rsheet.handle_command("set G1 1/0".to_string()).await;
        assert_eq!(reply, replies::Reply::Error("Division by zero".to_string()));

        let reply = rsheet.handle_command("set J1 1+2*3".to_string()).await;
        assert_eq!(reply, replies::Reply::Ok);

       
    }

}