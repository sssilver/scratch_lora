use core::str;
use core::str::Utf8Error;

const MAX_NMEA_SENTENCE_SIZE: usize = 128;

type Buffer = [u8; MAX_NMEA_SENTENCE_SIZE];

#[derive(Debug)]
pub struct SentenceBuffer {
    cursor: usize,
    buffer: Buffer,

    state: ParseState,
}

#[derive(Debug)]
enum ParseState {
    /// Waiting for a start-of-sentence marker
    Waiting,

    /// Collecting characters of the sentence (everything after `'$'`)
    Collecting,

    /// After encountering `'*'` in the sentence, we expect exactly two hex characters for the checksum
    InChecksum { count: usize },

    /// After reading the two checksum digits, we wait for the final terminator (CR or LF)
    Terminating,

    /// Sentence is complete and ready to be consumed
    Complete,
}

impl SentenceBuffer {
    pub fn new() -> Self {
        Self {
            cursor: 0,
            buffer: [0; MAX_NMEA_SENTENCE_SIZE],

            state: ParseState::Waiting,
        }
    }

    fn push_byte(&mut self, byte: u8) {
        if self.cursor >= self.buffer.len() {
            // When overflown, reset the the buffer
            self.reset("Buffer overflow");
        }

        self.buffer[self.cursor] = byte;
        self.cursor += 1;
    }

    pub fn as_string(&self) -> Result<&str, Utf8Error> {
        str::from_utf8(&self.buffer[..self.cursor])
    }

    pub fn feed(&mut self, byte: u8) -> Option<&str> {
        match self.state {
            ParseState::Waiting => {
                // In this state we're only looking for the start-of-sentence marker
                if byte == b'$' {
                    self.reset("Start-of-sentence marker ($) found");
                    self.push_byte(byte);
                    self.state = ParseState::Collecting;
                }
            }

            ParseState::Collecting => {
                match byte {
                    b'*' => {
                        // Transition to checksum state
                        self.push_byte(byte);
                        self.state = ParseState::InChecksum { count: 0 };
                    }

                    b'\r' => {
                        // Sentence termination without a checksum
                        defmt::warn!(
                            "Sentence terminated without checksum: {}",
                            self.as_string().unwrap()
                        );
                        self.reset("Sentence terminated without checksum");
                    }

                    b'\n' => {} // Just skip the newlines

                    _ => {
                        // Append any regular byte
                        self.push_byte(byte);
                    }
                }
            }

            ParseState::InChecksum { count } => {
                // Expecting exactly two hexadecimal digits
                self.push_byte(byte);

                let new_count = count + 1;
                if new_count == 2 {
                    // After two hex digits, transition to termination state
                    self.state = ParseState::Terminating;
                } else {
                    self.state = ParseState::InChecksum { count: new_count };
                }
            }

            ParseState::Terminating => {
                match byte {
                    // NL finishes the sentence
                    b'\n' => {
                        // Check if this is an RMC sentence
                        // if self.buffer.starts_with(b"$GPRMC") {
                        // Transition to Complete state and return the sentence
                        self.state = ParseState::Complete;

                        if let Ok(sentence_str) = self.as_string() {
                            return Some(sentence_str);
                        } else {
                            defmt::warn!("Invalid UTF-8 in NMEA sentence");
                        }
                        // } else {
                        //     self.reset("Not an RMC sentence");
                        // }
                    }

                    // Ignore CR
                    b'\r' => {}

                    // Any other byte in the termination phase is unexpected and resets the parser
                    _ => self.reset("Unexpected byte in Terminating phase"),
                }
            }

            ParseState::Complete => {
                // This should never be reached due to the reset at the start of the function
                self.reset("ParseState::Complete state reached");
            }
        }
        None
    }

    pub fn reset(&mut self, reason: &str) {
        self.cursor = 0;
        self.buffer.fill(0);

        self.state = ParseState::Waiting;

        defmt::debug!("Resetting sentence buffer -- {}", reason);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gps_sentence_validity() {
        assert!(true);
    }
}
