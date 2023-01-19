use argh::FromArgs;
use num_format::{Locale, ToFormattedString};
use regex::{Regex, Captures};
use std::{io::{Read, self, Write}, fs, time::Instant};

#[derive(FromArgs)]
#[argh(description="arguments")]
struct Args {
    #[argh(
        default="String::from(\"bf/primes.b\")",
        description="the bf script to run",
        option,
        short='l',
    )]
    load: String,
    #[argh(
        description="output debug information",
        short='d',
        switch,
    )]
    debug: bool,
    #[argh(
        description="verbose output debug information",
        switch,
    )]
    verbose: bool,
    #[argh(
        description="script input",
        option,
        short='i',
    )]
    input: Option<String>,
    #[argh(
        default="99",
        description="optimization level",
        option,
    )]
    level: usize,
    #[argh(
        description="always flush the output to stdout immediately. note that this can impact performance",
        short='F',
        switch,
    )]
    flush_immediate: bool,
}

#[derive(Debug,PartialEq)]
enum BfCommand {
    IncRef,
    IncVal,
    JumpBack,
    JumpTo,
    ScanTo,
    SetVal,
    StdIn,
    StdOut,
}

#[derive(Debug)]
struct BfInstruction {
    cmd: BfCommand,
    val: isize,
    offset: isize,
}
impl BfInstruction {
    fn new(cmd: BfCommand, val: isize) -> Self {
        BfInstruction { cmd, val, offset: 0 }
    }
}
impl TryFrom<char> for BfInstruction {
    type Error = &'static str;
    fn try_from(raw_cmd: char) -> Result<Self, Self::Error> {
        match raw_cmd {
            // standard
            '<' => Ok(BfInstruction::new(BfCommand::IncRef, -1)),
            '>' => Ok(BfInstruction::new(BfCommand::IncRef, 1)),
            '+' => Ok(BfInstruction::new(BfCommand::IncVal, 1)),
            '-' => Ok(BfInstruction::new(BfCommand::IncVal, -1)),
            '[' => Ok(BfInstruction::new(BfCommand::JumpTo, -1)),
            ']' => Ok(BfInstruction::new(BfCommand::JumpBack, -1)),
            '.' => Ok(BfInstruction::new(BfCommand::StdOut, 0)),
            ',' => Ok(BfInstruction::new(BfCommand::StdIn, 0)),
            // extended
            '=' => Ok(BfInstruction::new(BfCommand::SetVal, 0)),
            _ => {
                if raw_cmd.is_ascii_lowercase() {
                    Ok(BfInstruction::new(BfCommand::ScanTo, 0 - (raw_cmd as u8 - 97) as isize))
                }
                else if raw_cmd.is_ascii_uppercase() {
                    Ok(BfInstruction::new(BfCommand::ScanTo, (raw_cmd as u8 - 65) as isize))
                }
                else {
                    Err("Unknown Instruction")
                }
            }
        }
    }
}

fn main() {

    let mut instructions: Vec<BfInstruction> = Vec::new();
    let mut jumps: Vec<usize> = Vec::new();

    let args: Args = argh::from_env();
    let raw_file = fs::read_to_string(args.load).unwrap();
    let mut raw_bf = replace_all(&raw_file, r"[^+|\-|<|>|\.|,|\[|\]]", "");
    let raw_len = raw_bf.len();

    let mut input_queue: Vec<char> = Vec::new();
    if let Some(input_file) = args.input {
        if let Ok(raw_input) = fs::read_to_string(&input_file) {
            input_queue.append(&mut raw_input.chars().rev().collect::<Vec<char>>());
        }
        else {
            panic!("Cannot find {}", input_file);
        }
    }

    if args.level > 0 {
        raw_bf = replace_all(&raw_bf, r"\[\+\]", "=");
        raw_bf = replace_all(&raw_bf, r"\[\-\]", "=");
        raw_bf = replace_all2(&raw_bf, r"\[(<{1,27})\]", 'a');
        raw_bf = replace_all2(&raw_bf, r"\[(>{1,27})\]", 'A');
    }

    for val in raw_bf.chars() { 
        if let Ok(mut instruction) = BfInstruction::try_from(val) {
            let i = instructions.len();
            if instruction.cmd == BfCommand::JumpBack {
                let x = jumps.pop().unwrap();
                instructions[x] = BfInstruction::new(BfCommand::JumpTo, i as isize);
                instruction.val = x as isize;
            }
            else if instruction.cmd == BfCommand::JumpTo {
                jumps.push(i);
            }
            if args.level > 1 {
                if i > 0 && (instruction.cmd == BfCommand::IncRef || instruction.cmd == BfCommand::IncVal) {
                    if let Some(last_instruction) = instructions.get_mut(i - 1) {
                        if last_instruction.cmd == instruction.cmd {
                            last_instruction.offset += instruction.offset;
                            last_instruction.val += instruction.val;
                            continue;
                        }
                        // else if args.level > 2 {
                        //     if last_instruction.cmd == BfCommand::IncRef && instruction.cmd == BfCommand::IncVal {

                        //     }
                        // }
                    }
                }
            }
            instructions.push(instruction);
        }
    }

    if args.verbose {
        for ins in &instructions {
            println!("{:?}", ins);
        }
    }

    let mut memory: Vec<u8> = vec![0; u16::MAX.into()];
    let mut mem_ptr = 0;
    let mut ins_ptr = 0;
    let mut count: usize = 0;
    let ins_len = instructions.len();
    let now = Instant::now();

    while ins_ptr < ins_len {
        let ins = &instructions[ins_ptr];
        match ins.cmd {
            BfCommand::IncRef => {
                mem_ptr += ins.val;
                if mem_ptr < 0 {
                    panic!("Invalid Address! {}", mem_ptr);
                }
            },
            BfCommand::IncVal => {
                let ptr = mem_ptr + ins.offset;
                let val = memory[ptr as usize] as isize + ins.val;
                memory[mem_ptr as usize] = val as u8;
            },
            BfCommand::JumpBack => {
                if memory[mem_ptr as usize] != 0 {
                    ins_ptr = ins.val as usize;
                }
            },
            BfCommand::JumpTo => {
                if memory[mem_ptr as usize] == 0 {
                    ins_ptr = ins.val as usize;
                }
            },
            BfCommand::StdIn => {
                let mut input = [0; 1];
                if input_queue.len() > 0 {
                    let c = input_queue.pop().unwrap();
                    input[0] = c as u8;
                }
                else {
                    let mut stdin = io::stdin();
                    stdin.read(&mut input[..]).unwrap();
                }
                let ptr = mem_ptr + ins.offset;
                memory[ptr as usize] = input[0];
            },
            BfCommand::StdOut => {
                let ptr = mem_ptr + ins.offset;
                let val = memory[ptr as usize];
                let c = val as char;
                print!("{}", c);
                if args.flush_immediate {
                    io::stdout().flush().unwrap();
                }
            },
            BfCommand::SetVal => {
                let ptr = mem_ptr + ins.offset;
                memory[ptr as usize] = ins.val as u8;
            },
            BfCommand::ScanTo => {
                let mut ptr = 0;
                while memory[(mem_ptr + ptr) as usize] != 0 {
                    ptr += ins.val;
                }
                mem_ptr += ptr;
            },
        }

        ins_ptr += 1;
        count += 1;

    }

    println!();

    if args.debug {
        let elapsed = Instant::elapsed(&now);
        println!("{} raw instructions", raw_len.to_formatted_string(&Locale::en));
        println!("{} compiled instructions", instructions.len().to_formatted_string(&Locale::en));
        println!("{} clock cycles executed in {:?}", count.to_formatted_string(&Locale::en), elapsed);
    }

}

fn replace_all(
    text: &str,
    exp: &str,
    rep: &str,
) -> String {
    Regex::new(&exp)
        .unwrap()
        .replace_all(text, rep)
        .to_string()
}

fn replace_all2(
    text: &str,
    exp: &str,
    starter: char,
) -> String {
    Regex::new(&exp)
        .unwrap()
        .replace_all(text, |caps: &Captures| {
            ((starter as u8 + caps.get(1).unwrap().as_str().len() as u8) as char).to_string()
        })
        .to_string()
}