#!/usr/bin/env python3
"""
Python example for parsing audible-util machine-readable output.

This script demonstrates how to parse the JSON progress events emitted by
audible-util when run with the --machine-readable flag.

Usage:
    python3 python_parser.py
"""

import json
import subprocess
import sys
import time
from typing import Dict, Any, Optional


class AudibleUtilParser:
    """Parser for audible-util machine-readable output."""
    
    def __init__(self):
        self.current_chapter = 0
        self.total_chapters = 0
        self.start_time = None
        self.conversion_success = False
        
    def parse_event(self, line: str) -> Optional[Dict[str, Any]]:
        """Parse a single JSON event line."""
        try:
            return json.loads(line.strip())
        except json.JSONDecodeError:
            return None
    
    def handle_event(self, event: Dict[str, Any]) -> None:
        """Handle a parsed progress event."""
        event_type = event.get("type")
        
        if event_type == "conversion_started":
            self.total_chapters = event.get("total_chapters", 0)
            output_format = event.get("output_format", "unknown")
            output_path = event.get("output_path", "unknown")
            print(f"🚀 Conversion started: {self.total_chapters} chapters to {output_format} format")
            print(f"📁 Output path: {output_path}")
            self.start_time = time.time()
            
        elif event_type == "chapter_started":
            chapter_num = event.get("chapter_number", 0)
            chapter_title = event.get("chapter_title", "Unknown")
            duration = event.get("duration_seconds", 0)
            self.current_chapter = chapter_num
            print(f"\n📖 Chapter {chapter_num}/{self.total_chapters}: {chapter_title}")
            print(f"⏱️  Duration: {duration:.1f} seconds")
            
        elif event_type == "chapter_progress":
            chapter_num = event.get("chapter_number", 0)
            progress_pct = event.get("progress_percentage", 0)
            current_time = event.get("current_time", 0)
            total_duration = event.get("total_duration", 0)
            speed = event.get("speed", 0)
            bitrate = event.get("bitrate", 0)
            file_size = event.get("file_size", 0)
            eta = event.get("eta_seconds")
            
            # Create progress bar
            bar_length = 40
            filled_length = int(bar_length * progress_pct / 100)
            bar = "█" * filled_length + "░" * (bar_length - filled_length)
            
            # Format file size
            size_str = self.format_file_size(file_size)
            
            # Format ETA
            eta_str = f"{eta:.0f}s" if eta else "Unknown"
            
            print(f"\r  {bar} {progress_pct:5.1f}% | Speed: {speed:.1f}x | Bitrate: {bitrate/1000:.0f}kbps | Size: {size_str} | ETA: {eta_str}", end="", flush=True)
            
        elif event_type == "chapter_completed":
            chapter_num = event.get("chapter_number", 0)
            chapter_title = event.get("chapter_title", "Unknown")
            output_file = event.get("output_file", "Unknown")
            duration = event.get("duration_seconds", 0)
            print(f"\n✅ Chapter {chapter_num} completed: {chapter_title}")
            print(f"📄 Output: {output_file}")
            print(f"⏱️  Duration: {duration:.1f} seconds")
            
        elif event_type == "conversion_completed":
            total_duration = event.get("total_duration_seconds", 0)
            success = event.get("success", False)
            self.conversion_success = success
            
            if success:
                print(f"\n🎉 Conversion completed successfully!")
                print(f"⏱️  Total time: {total_duration:.1f} seconds")
            else:
                print(f"\n❌ Conversion failed!")
                
        elif event_type == "error":
            message = event.get("message", "Unknown error")
            chapter_num = event.get("chapter_number")
            if chapter_num:
                print(f"\n❌ Error in chapter {chapter_num}: {message}")
            else:
                print(f"\n❌ Error: {message}")
    
    def format_file_size(self, size_bytes: int) -> str:
        """Format file size in human-readable format."""
        if size_bytes == 0:
            return "0 B"
        
        units = ["B", "KB", "MB", "GB"]
        size = float(size_bytes)
        unit_index = 0
        
        while size >= 1024 and unit_index < len(units) - 1:
            size /= 1024
            unit_index += 1
        
        return f"{size:.1f} {units[unit_index]}"
    
    def run_conversion(self, args: list) -> int:
        """Run audible-util with machine-readable output and parse events."""
        print("🎵 Starting audible-util conversion with machine-readable output...")
        print("=" * 60)
        
        try:
            # Run audible-util with machine-readable flag
            process = subprocess.Popen(
                args + ["--machine-readable"],
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                bufsize=1,
                universal_newlines=True
            )
            
            # Parse output line by line
            for line in process.stdout:
                event = self.parse_event(line)
                if event:
                    self.handle_event(event)
            
            # Wait for process to complete
            return_code = process.wait()
            
            if return_code != 0:
                stderr_output = process.stderr.read()
                print(f"\n❌ Process failed with return code {return_code}")
                print(f"Error output: {stderr_output}")
                return return_code
            
            return 0
            
        except Exception as e:
            print(f"\n❌ Error running audible-util: {e}")
            return 1


def main():
    """Main function."""
    if len(sys.argv) < 2:
        print("Usage: python3 python_parser.py <audible-util-args>")
        print("Example: python3 python_parser.py -a book.aaxc -v book.voucher -s")
        sys.exit(1)
    
    # Get audible-util arguments
    audible_args = sys.argv[1:]
    
    # Create parser and run conversion
    parser = AudibleUtilParser()
    return_code = parser.run_conversion(audible_args)
    
    sys.exit(return_code)


if __name__ == "__main__":
    main()
