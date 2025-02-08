use embassy_time::Instant;

defmt::timestamp!("({=u32:us})", Instant::now().as_micros() as u32);
