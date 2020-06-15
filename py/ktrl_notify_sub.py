#!/usr/bin/python3
import zmq
import argparse

DEFAULT_NOTIFY_PORT = 7333

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", help="ktrl's notify port")
    args = parser.parse_args()

    if args.port == None:
        port = DEFAULT_NOTIFY_PORT
    else:
        port = int(args.port)

    context = zmq.Context()
    endpoint = "tcp://127.0.0.1:" + str(port)
    socket = context.socket(zmq.SUB)
    socket.connect(endpoint)
    socket.setsockopt(zmq.SUBSCRIBE, b"layer")
    print("Connected to ktrl's notification server: " + endpoint)

    while True:
        message = socket.recv()
        print("NOTIFY: [ %s ]" % message)


if __name__ == "__main__":
    main()
