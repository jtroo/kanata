#!/usr/bin/python3
import zmq
import sys

def main():
    if len(sys.argv) == 0:
        port = 7331
    else:
        port = sys.argv[1]

    context = zmq.Context()

    #  Socket to talk to server
    endpoint = "tcp://127.0.0.1:" + str(port)
    print("Connecting to ktrl's ipc server: " + endpoint)
    socket = context.socket(zmq.REQ)
    socket.connect(endpoint)

    req = b"IpcDoEffect((fx: NoOp, val: Press))"
    print("Sending request %s" % req)
    socket.send(req)

    #  Get the reply.
    message = socket.recv()
    print("Received reply [ %s ]" % message)


if __name__ == "__main__":
    main()
