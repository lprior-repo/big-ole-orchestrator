import re, glob

variants = [
    "InvalidInput", "InvalidEnvelopeFormat", "MissingEnvelopeField", "InvalidEnvelopeField",
    "UnsupportedEnvelopeVersion", "MissingSchemaVersion", "InvalidSchemaVersionFormat",
    "UnsupportedSchemaVersion", "InvalidPayloadFormat", "MissingPayloadField",
    "InvalidPayloadField", "UnsupportedPayloadVersion", "UnknownPayloadType",
    "EnvelopeDecodeFailed", "PayloadDecodeSkipped", "PayloadDecodeFailed", "SerializationError"
]

content = ""
for f in glob.glob("crates/vo-types/src/**/*.rs", recursive=True) + glob.glob("crates/vo-types/tests/**/*.rs", recursive=True):
    with open(f) as file:
        content += file.read()

for v in variants:
    if v not in content:
        print(f"MISSING VARIANT IN TESTS: {v}")
