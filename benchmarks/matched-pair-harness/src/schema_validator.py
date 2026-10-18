"""
Zero-dependency lightweight JSON schema validator for Harness schemas.
Supports core draft-2020-12 constructs: type, required, properties, items, enum, const, min/max.
"""
from typing import Any, Dict, List, Optional, Union
import json
from pathlib import Path


class SchemaValidationError(Exception):
    def __init__(self, path: str, message: str):
        super().__init__(f"Validation error at '{path}': {message}")
        self.path = path
        self.message = message


def validate_instance(instance: Any, schema: Dict[str, Any], path: str = "$") -> None:
    # 1. Type validation
    if "type" in schema:
        expected_type = schema["type"]
        if isinstance(expected_type, list):
            valid = False
            for t in expected_type:
                if _check_type(instance, t):
                    valid = True
                    break
            if not valid:
                raise SchemaValidationError(path, f"Expected one of types {expected_type}, got {type(instance).__name__}")
        else:
            if not _check_type(instance, expected_type):
                raise SchemaValidationError(path, f"Expected type '{expected_type}', got {type(instance).__name__}")

    # 2. Const validation
    if "const" in schema:
        if instance != schema["const"]:
            raise SchemaValidationError(path, f"Expected const '{schema['const']}', got '{instance}'")

    # 3. Enum validation
    if "enum" in schema:
        if instance not in schema["enum"]:
            raise SchemaValidationError(path, f"Value '{instance}' not in enum {schema['enum']}")

    # 4. Numeric bounds
    if isinstance(instance, (int, float)) and not isinstance(instance, bool):
        if "minimum" in schema and instance < schema["minimum"]:
            raise SchemaValidationError(path, f"Value {instance} is less than minimum {schema['minimum']}")
        if "maximum" in schema and instance > schema["maximum"]:
            raise SchemaValidationError(path, f"Value {instance} is greater than maximum {schema['maximum']}")

    # 5. Object validation
    if isinstance(instance, dict):
        required_keys = schema.get("required", [])
        for key in required_keys:
            if key not in instance:
                raise SchemaValidationError(path, f"Missing required property '{key}'")

        properties = schema.get("properties", {})
        for key, value in instance.items():
            if key in properties:
                validate_instance(value, properties[key], f"{path}.{key}")
            elif "additionalProperties" in schema:
                add_prop = schema["additionalProperties"]
                if isinstance(add_prop, dict):
                    validate_instance(value, add_prop, f"{path}.{key}")
                elif add_prop is False:
                    raise SchemaValidationError(path, f"Additional property '{key}' is not allowed")

    # 6. Array validation
    if isinstance(instance, list):
        if "items" in schema and isinstance(schema["items"], dict):
            for i, item in enumerate(instance):
                validate_instance(item, schema["items"], f"{path}[{i}]")


def _check_type(instance: Any, type_str: str) -> bool:
    if type_str == "object":
        return isinstance(instance, dict)
    elif type_str == "array":
        return isinstance(instance, list)
    elif type_str == "string":
        return isinstance(instance, str)
    elif type_str == "integer":
        return isinstance(instance, int) and not isinstance(instance, bool)
    elif type_str == "number":
        return isinstance(instance, (int, float)) and not isinstance(instance, bool)
    elif type_str == "boolean":
        return isinstance(instance, bool)
    elif type_str == "null":
        return instance is None
    return True


def validate_file(json_path: Union[str, Path], schema_path: Union[str, Path]) -> None:
    with open(json_path, "r", encoding="utf-8") as f:
        data = json.load(f)
    with open(schema_path, "r", encoding="utf-8") as f:
        schema = json.load(f)
    validate_instance(data, schema)
