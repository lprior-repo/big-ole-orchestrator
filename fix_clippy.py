import re

path = "crates/vo-storage/src/codec.rs"
with open(path, "r") as f:
    content = f.read()

content = content.replace("fn mixed_id() -> InstanceId {", "#[allow(dead_code)]\n    fn mixed_id() -> InstanceId {")
content = content.replace("mod tests {", "#[allow(clippy::unwrap_used)]\nmod tests {")
content = content.replace("mod proptests {", "#[allow(clippy::unwrap_used)]\nmod proptests {")
content = content.replace("#[cfg(kani)]\nmod verification {", "#[cfg(kani)]\n#[allow(unexpected_cfgs)]\nmod verification {")
content = content.replace("0x0102030405060708u64", "0x0102_0304_0506_0708_u64")

with open(path, "w") as f:
    f.write(content)
