Fork [xiu](https://github.com/harlanc/xiu)


### Coturn Usage

- generate `cert` and `pkey`: `openssl req -x509 -newkey rsa:1024 -keyout /tmp/turn_key.pem -out /tmp/turn_cert.pem -days 9999 -nodes`

- edit configure
    ```bash
    # 默认位置：/etc/turnserver.conf 或 /etc/coturn/turnserver.conf
    listening-ip=0.0.0.0
    listening-port=3478
    relay-ip=192.168.10.8
    external-ip=192.168.10.8

    tls-listening-port=5349
    cert=/tmp/turn_cert.pem
    pkey=/tmp/turn_key.pem

    realm=example.com

    # lt-cred-mech
    # user=foo:123456

    no-auth
    no-cli
    verbose
    ```

- test `turnserver`
    -  turnserver -c /etc/turnserver.conf -v
    -  visit [Trickle ICE](https://webrtc.github.io/samples/src/content/peerconnection/trickle-ice/)
    - `turn` server uri format: `turn:ip:3478`


### Links
- [Turn Server Test](https://turndemo.metered.ca/)
- [cuturn](https://github.com/coturn/coturn) is `stun` and `turn` server
- [WebRTC samples Trickle ICE](https://webrtc.github.io/samples/src/content/peerconnection/trickle-ice/)
- [WebRTC - STUN/TURN服务器的搭建（使用coturn）](https://www.cnblogs.com/brisk/p/17033862.html)
