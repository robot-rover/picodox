import os

DEVICE = "/dev/ttyACM0"

# fd = os.open(DEVICE, os.O_RDWR | os.O_NOCTTY)
# os.write(fd, b'Hello World\r')
# for i in range(10):
#     print(os.read(fd, 16))
# os.close(fd)

import serial

ser = serial.Serial()
ser.port = DEVICE
ser.open()
ser.write(b'Hello World\r')
ser.flush()
while True:
    print(ser.read())
