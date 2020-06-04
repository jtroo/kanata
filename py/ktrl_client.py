#!/usr/bin/python3
import zmq
import argparse

DEFAULT_IPC_PORT = 7331

#
# Example Usage:
# --------------
#
# ./ktrl_client.py --port 123456 "IpcDoEffect((fx: NoOp, val: Press))"
# ./ktrl_client.py "IpcDoEffect((fx: NoOp, val: Press))"
#

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", help="ktrl's ipc port")
    parser.add_argument("msg", help="ipc msg to send to ktrl")
    args = parser.parse_args()

    if args.port == None:
        port = DEFAULT_IPC_PORT
    else:
        port = int(args.port)

    context = zmq.Context()

    endpoint = "tcp://127.0.0.1:" + str(port)
    print("Connecting to ktrl's ipc server: " + endpoint)
    socket = context.socket(zmq.REQ)
    socket.connect(endpoint)

    print("Sending request %s" % args.msg)
    socket.send_string(args.msg)

    message = socket.recv()
    print("Received reply [ %s ]" % message)


if __name__ == "__main__":
    main()
