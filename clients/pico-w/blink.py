import socket
import time

import network
import tls
from machine import Pin

host = "oubot.kittyandrew.dev"
token = "..."
led = Pin("LED", Pin.OUT)
ssid = "wifi-name-2.4"
password = "..."


def urlopen(url, token=None, method="GET"):
    try:
        proto, _, host, path = url.split("/", 3)
    except ValueError:
        proto, _, host = url.split("/", 2)
        path = ""

    if not proto.startswith("http"):
        raise ValueError("Unsupported protocol: " + proto)

    port = 443 if proto == "https:" else 80

    if ":" in host:
        host, port = host.split(":", 1)
        port = int(port)

    ai = socket.getaddrinfo(host, port, 0, socket.SOCK_STREAM)
    ai = ai[0]

    s = socket.socket(ai[0], ai[1], ai[2])
    try:
        s.settimeout(3)
        s.connect(ai[-1])
        if proto == "https:":
            context = tls.SSLContext(tls.PROTOCOL_TLS_CLIENT)
            context.verify_mode = tls.CERT_NONE
            s = context.wrap_socket(s, server_hostname=host)

        s.write(method)
        s.write(b" /")
        s.write(path)
        s.write(b" HTTP/1.0\r\nHost: ")
        s.write(host)
        s.write(b"\r\n")

        if token:
            s.write(b"Authorization: ")
            s.write(token)
            s.write(b"\r\n")
        s.write(b"\r\n")

        return s.readline()  # Status-Line
    finally:
        s.close()


def main():
    while True:
        try:
            led.on()
            wlan = network.WLAN(network.STA_IF)
            wlan.active(True)
            wlan.connect(ssid, password)
        except Exception as e:
            print("WLAN Err:", e)
            time.sleep(4)
            led.off()
            continue

        while True:
            led.on()
            if wlan.status() < 0 or wlan.status() >= 3:
                break
            time.sleep(1)
            led.off()

        if wlan.status() != 3:
            led.on()
            print("WLAN: network connection failed")
            time.sleep(10)
            led.off()
            continue

        print(f"connected (ip = {wlan.ifconfig()[0]})")
        while True:
            led.on()
            try:
                r = urlopen(f"https://{host}/api/v1/up", token=token)
                print("Res:", r)
                time.sleep(1)
            except Exception as e:
                print("REQ Err:", e)
                time.sleep(4)

            led.off()
            time.sleep(4)


if __name__ == "__main__":
    main()
