import sys
import os
import tempfile

print("Python executable:", sys.executable)
print("Is frozen?", getattr(sys, 'frozen', False))
print("Script location:", __file__ if '__file__' in locals() else 'No __file__')
print("Working directory:", os.getcwd())
print("Temp dir:", tempfile.gettempdir())

# Test creating a file in temp
test_file = os.path.join(tempfile.gettempdir(), "test_write.txt")
try:
    with open(test_file, 'w') as f:
        f.write("Test write successful")
    print(f"Successfully wrote to: {test_file}")
    os.unlink(test_file)
except Exception as e:
    print(f"Failed to write: {e}")