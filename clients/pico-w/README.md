
1. Plug pico in with button pressed (in bootsel mode)
2. Download firmware from https://micropython.org/download/RPI_PICO_W/
3. Load firmware to the device (e.g. `sudo picotool load RPI_PICO_W-20241025-v1.24.0.uf2`)
4. Plug pico in with**out** button pressed.
5. Run `sudo minicom -o -D /dev/ttyACM0 -b 115200` to connect to the micropython shell on the device. This should drop you into python shell on the device, if it succeeded, you can exit by pressing `ctrl+A` then `x` and confirm.
6. Deploy `sudo rshell -p /dev/ttyACM0 --buffer-size 512 cp blink.py /pyboard/main.py`
7. Plug it off and on or reboot the device for the program to run.

