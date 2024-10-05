use serde_json::Value;

pub fn read_json(path: &str) -> Result<Value, anyhow::Error> {
    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);
    let value = serde_json::from_reader(reader)?;
    Ok(value)
}

pub fn write_json(path: &str, value: &Value) -> Result<(), anyhow::Error> {
    let file = std::fs::File::create(path)?;
    let writer = std::io::BufWriter::new(file);
    serde_json::to_writer(writer, value)?;
    Ok(())
}
