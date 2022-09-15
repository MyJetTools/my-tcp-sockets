## 0.1.3
* Uses Logger Trait to do logging and it injects via Start;
* SocketConnection now have Send Buffer;
* Send Operation now has Timeouts;

## 0.1.4
* We can stop Client Socket - it's not connected to Applications states anymore;
* Fixed bug - if application is not initialized - but we establish client socket - it would not send payloads;
* We have timeout on read loop;

## 0.1.5
Now we can mesure the ping/pong roundtrip duration;

## 0.1.6
Using Tags System

## 0.1.7
* Client connection settings are read each connection;