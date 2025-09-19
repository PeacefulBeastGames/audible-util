#!/usr/bin/env python3
"""
Simple Python example for parsing audible-util machine-readable output.

This script shows how to parse the raw JSON events from audible-util.

Usage:
    python3 simple_json_parser.py
"""

import json
import subprocess
import sys


def main():
    """Parse audible-util machine-readable output."""
    if len(sys.argv) < 2:
        print("Usage: python3 simple_json_parser.py <audible-util-args>")
        print("Example: python3 simple_json_parser.py -a book.aaxc -v book.voucher")
        sys.exit(1)
    
    # Get audible-util arguments
    audible_args = sys.argv[1:]
    
    try:
        # Run audible-util with machine-readable flag
        process = subprocess.Popen(
            audible_args + ["--machine-readable"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True
        )
        
        # Parse each line of JSON output
        for line in process.stdout:
            line = line.strip()
            if line:
                try:
                    event = json.loads(line)
                    print(f"Event: {json.dumps(event, indent=2)}")
                except json.JSONDecodeError as e:
                    print(f"Failed to parse JSON: {line} - Error: {e}")
        
        # Wait for process to complete
        return_code = process.wait()
        
        if return_code != 0:
            stderr_output = process.stderr.read()
            print(f"Process failed with return code {return_code}")
            print(f"Error output: {stderr_output}")
        
        sys.exit(return_code)
        
    except Exception as e:
        print(f"Error running audible-util: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()
