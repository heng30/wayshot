const Extensions = {
  Core: {
    ServerSentEvents: "urn:ietf:params:whep:ext:core:server-sent-events",
    Layer: "urn:ietf:params:whep:ext:core:layer",
  },
};

class WHEPClient extends EventTarget {
  constructor() {
    super();
    //Ice properties
    this.iceUsername = null;
    this.icePassword = null;
    //Pending candidadtes
    this.candidates = [];
    this.endOfcandidates = false;
  }

  async fetchIceServers(mediainfoUrl, token) {
    try {
      const headers = {};
      if (token) headers["Authorization"] = "Bearer " + token;

      const response = await fetch(mediainfoUrl, {
        method: "GET",
        headers,
      });

      if (!response.ok) {
        console.warn(
          "Failed to fetch ICE servers from /mediainfo:",
          response.status,
        );
        return [];
      }

      const data = await response.json();

      // Extract ICE servers from the mediainfo response
      // Expected format: { ice_servers: ["stun:example.com:443", "turn:example.com:443", ...] }
      if (data.ice_servers && Array.isArray(data.ice_servers)) {
        return data.ice_servers.map((url) => ({ urls: url }));
      }

      return [];
    } catch (error) {
      console.warn("Error fetching ICE servers:", error);
      return [];
    }
  }

  async view(pc, url, token) {
    //If already publishing
    if (this.pc) throw new Error("Already viewing");

    //Store pc object and token
    this.token = token;
    this.pc = pc;

    //Listen for state change events
    pc.onconnectionstatechange = (event) => {
      switch (pc.connectionState) {
        case "connected":
          // The connection has become fully connected
          break;
        case "disconnected":
        case "failed":
          // One or more transports has terminated unexpectedly or in an error
          break;
        case "closed":
          // The connection has been closed
          break;
      }
    };

    //Listen for candidates
    pc.onicecandidate = (event) => {
      if (event.candidate) {
        //Ignore candidates not from the first m line
        if (event.candidate.sdpMLineIndex > 0)
          //Skip
          return;
        //Store candidate
        this.candidates.push(event.candidate);
      } else {
        //No more candidates
        this.endOfcandidates = true;
      }
      //Schedule trickle on next tick
      if (!this.iceTrickeTimeout)
        this.iceTrickeTimeout = setTimeout(() => this.trickle(), 0);
    };

    // Fetch ICE servers from /mediainfo API
    const mediainfoUrl = new URL("/mediainfo", url).toString();
    const iceServers = await this.fetchIceServers(mediainfoUrl, token);

    // Set ICE servers configuration if we have any
    if (iceServers.length > 0) {
      const config = pc.getConfiguration();
      config.iceServers = iceServers;
      pc.setConfiguration(config);
      console.log("Set ICE servers from /mediainfo:", iceServers);
    }

    //Create SDP offer
    const offer = await pc.createOffer();

    //Request headers
    const headers = {
      "Content-Type": "application/sdp",
    };

    //If token is set
    if (token) headers["Authorization"] = "Bearer " + token;

    //Do the post request to the WHEP endpoint with the SDP offer
    const fetched = await fetch(url, {
      method: "POST",
      body: offer.sdp,
      headers,
    });

    if (!fetched.ok) {
      if (fetched.status === 401) {
        throw new Error("UNAUTHORIZED");
      } else {
        throw new Error("Request rejected with status " + fetched.status);
      }
    }
    if (!fetched.headers.get("location"))
      throw new Error("Response missing location header");

    //Get the resource url
    this.resourceURL = new URL(fetched.headers.get("location"), url);

    //Get the links
    const links = {};

    //If the response contained any
    if (fetched.headers.has("link")) {
      //Get all links headers
      const linkHeaders = fetched.headers.get("link").split(/,\s+(?=<)/);

      //For each one
      for (const header of linkHeaders) {
        try {
          let rel,
            params = {};
          //Split in parts
          const items = header.split(";");
          //Create url server
          const url = items[0]
            .trim()
            .replace(/<(.*)>/, "$1")
            .trim();
          //For each other item
          for (let i = 1; i < items.length; ++i) {
            //Split into key/val
            const subitems = items[i].split(/=(.*)/);
            //Get key
            const key = subitems[0].trim();
            //Unquote value
            const value = subitems[1]
              ? subitems[1].trim().replaceAll('"', "").replaceAll("'", "")
              : subitems[1];
            //Check if it is the rel attribute
            if (key == "rel")
              //Get rel value
              rel = value;
            else
              //Unquote value and set them
              params[key] = value;
          }
          //Ensure it is an ice server
          if (!rel) continue;
          if (!links[rel]) links[rel] = [];
          //Add to config
          links[rel].push({ url, params });
        } catch (e) {
          console.error(e);
        }
      }
    }

    //Get extensions url
    if (links.hasOwnProperty(Extensions.Core.ServerSentEvents))
      //Get url
      this.eventsUrl = new URL(
        links[Extensions.Core.ServerSentEvents][0].url,
        url,
      );
    if (links.hasOwnProperty(Extensions.Core.Layer))
      this.layerUrl = new URL(links[Extensions.Core.Layer][0].url, url);

    //If we have an event url
    if (this.eventsUrl) {
      //Get supported events
      const events = links[Extensions.Core.ServerSentEvents]["events"]
        ? links[Extensions.Core.ServerSentEvents]["events"].split(" ")
        : ["active", "inactive", "layers", "viewercount"];
      //Request headers
      const headers = {
        "Content-Type": "application/json",
      };

      //If token is set
      if (this.token) headers["Authorization"] = "Bearer " + this.token;

      //Do the post request to the whep resource
      fetch(this.eventsUrl, {
        method: "POST",
        body: JSON.stringify(events),
        headers,
      }).then((fetched) => {
        //If the event channel could be created
        if (!fetched.ok) return;
        //Get the resource url
        const sseUrl = new URL(fetched.headers.get("location"), this.eventsUrl);
        //Open it
        this.eventSource = new EventSource(sseUrl);
        this.eventSource.onopen = (event) => console.log(event);
        this.eventSource.onerror = (event) => console.log(event);
        //Listen for events
        this.eventSource.onmessage = (event) => {
          console.dir(event);
          this.dispatchEvent(event);
        };
      });
    }

    //Get current config
    const config = pc.getConfiguration();

    //If it has ice server info and it is not overriden by the client
    if (
      (!config.iceServer || !config.iceServer.length) &&
      links.hasOwnProperty("ice-server")
    ) {
      //ICe server config
      config.iceServers = [];

      //For each one
      for (const server of links["ice-server"]) {
        try {
          //Create ice server
          const iceServer = {
            urls: server.url,
          };
          //For each other param
          for (const [key, value] of Object.entries(server.params)) {
            //Get key in cammel case
            const cammelCase = key.replace(/([-_][a-z])/gi, ($1) =>
              $1.toUpperCase().replace("-", "").replace("_", ""),
            );
            //Unquote value and set them
            iceServer[cammelCase] = value;
          }
          //Add to config
          //config.iceServers.push(iceServer);
        } catch (e) {}
      }

      //If any configured
      if (config.iceServers.length)
        //Set it
        pc.setConfiguration(config);
    }

    //Get the SDP answer
    const answer = await fetched.text();

    //Schedule trickle on next tick
    if (!this.iceTrickeTimeout)
      this.iceTrickeTimeout = setTimeout(() => this.trickle(), 0);

    //Set local description
    await pc.setLocalDescription(offer);

    // TODO: chrome is returning a wrong value, so don't use it for now
    //try {
    //	//Get local ice properties
    //	const local = this.pc.getTransceivers()[0].sender.transport.iceTransport.getLocalParameters();
    //	//Get them for transport
    //	this.iceUsername = local.usernameFragment;
    //	this.icePassword = local.password;
    //} catch (e) {
    //Fallback for browsers not supporting ice transport
    this.iceUsername = offer.sdp.match(/a=ice-ufrag:(.*)\r\n/)[1];
    this.icePassword = offer.sdp.match(/a=ice-pwd:(.*)\r\n/)[1];
    //}

    //And set remote description
    await pc.setRemoteDescription({ type: "answer", sdp: answer });
  }

  restart() {
    //Set restart flag
    this.restartIce = true;

    //Schedule trickle on next tick
    if (!this.iceTrickeTimeout)
      this.iceTrickeTimeout = setTimeout(() => this.trickle(), 0);
  }

  async trickle() {
    // 如果没有候选者或 URL 不存在，直接返回
    if (this.candidates.length === 0 || !this.resourceURL) return;

    // 复制并清空当前队列，防止并发问题
    const candidatesToSend = [...this.candidates];
    this.candidates = [];

    // 如果已经没有更多候选者了，WHEP 建议发送一个空片段或特定标识，
    // 但通常只要把收集到的发过去即可。

    // 假设 WHEP 资源 URL 支持 PATCH 方法来接收 ICE 候选者
    // (注意：不同的 WHEP 实现可能对 Trickle ICE 的处理不同，
    // 有的是 PATCH请求，有的是 POST 到特定链接。你需要确认 whep.rs 如何处理)

    // 这里是一个通用的 PATCH 实现示例：
    try {
      const headers = {
        "Content-Type": "application/trice", // 或 application/json-patch+json
      };
      if (this.token) headers["Authorization"] = "Bearer " + this.token;

      // 构建 WHEP 标准的 body (JSON Fragment)
      // 注意：具体格式取决于你的 Rust 服务器实现，这里假设直接发 candidate 数组或特定结构
      // 标准 WHEP 通常使用 PATCH 请求发送 candidate
      await fetch(this.resourceURL, {
        method: "PATCH",
        headers: headers,
        body: JSON.stringify(candidatesToSend),
      });
    } catch (e) {
      console.error("Trickle failed", e);
    }
  }

  async mute(muted) {
    //Request headers
    const headers = {
      "Content-Type": "application/json",
    };

    //If token is set
    if (this.token) headers["Authorization"] = "Bearer " + this.token;

    //Do the post request to the whep resource
    const fetched = await fetch(this.resourceURL, {
      method: "POST",
      body: JSON.stringify(muted),
      headers,
    });
  }

  async selectLayer(layer) {
    if (!this.layerUrl)
      throw new Error("whep resource does not support layer selection");

    //Request headers
    const headers = {
      "Content-Type": "application/json",
    };

    //If token is set
    if (this.token) headers["Authorization"] = "Bearer " + this.token;

    //Do the post request to the whep resource
    const fetched = await fetch(this.layerUrl, {
      method: "POST",
      body: JSON.stringify(layer),
      headers,
    });
  }

  async unselectLayer() {
    if (!this.layerUrl)
      throw new Error("whep resource does not support layer selection");

    //Request headers
    const headers = {};

    //If token is set
    if (this.token) headers["Authorization"] = "Bearer " + this.token;

    //Do the post request to the whep resource
    const fetched = await fetch(this.layerUrl, {
      method: "DELETE",
      headers,
    });
  }

  async stop() {
    if (!this.pc) {
      // Already stopped
      return;
    }

    //Cancel any pending timeout
    this.iceTrickeTimeout = clearTimeout(this.iceTrickeTimeout);

    //Close peerconnection
    this.pc.close();

    //Null
    this.pc = null;

    //If we don't have the resource url
    if (!this.resourceURL)
      throw new Error("WHEP resource url not available yet");

    //Request headers
    const headers = {};

    //If token is set
    if (this.token) headers["Authorization"] = "Bearer " + this.token;

    //Send a delete
    await fetch(this.resourceURL, {
      method: "DELETE",
      headers,
    });
  }
}
