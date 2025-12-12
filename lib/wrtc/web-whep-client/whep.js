const Extensions = {
  Core: {
    ServerSentEvents: "urn:ietf:params:whep:ext:core:server-sent-events",
    Layer: "urn:ietf:params:whep:ext:core:layer",
  },
};

class WHEPClient extends EventTarget {
  constructor() {
    super();
    this.pc = null;
    this.resourceURL = null;
    this.token = null;
    this.iceServers = [];
  }

  /**
   * 获取 ICE 服务器配置
   * 包含错误处理和备选方案
   */
  async fetchIceServers(mediainfoUrl, token) {
    try {
      const headers = {};
      if (token) headers["Authorization"] = "Bearer " + token;

      console.log("Fetching ICE servers from:", mediainfoUrl);

      const response = await fetch(mediainfoUrl, {
        method: "GET",
        headers,
      });

      if (!response.ok) {
        console.warn(
          `Failed to fetch ICE servers from /mediainfo: ${response.status} ${response.statusText}`,
        );
        return this.getDefaultIceServers();
      }

      const data = await response.json();
      console.log("Received ICE servers data:", JSON.stringify(data, null, 2));

      // 处理各种可能的响应格式
      if (data.ice_servers && Array.isArray(data.ice_servers)) {
        this.iceServers = this.normalizeIceServers(data.ice_servers);
        console.log("Normalized ICE servers:", this.iceServers);
        return this.iceServers;
      }

      // 尝试其他可能的字段名
      if (data.iceServers && Array.isArray(data.iceServers)) {
        this.iceServers = this.normalizeIceServers(data.iceServers);
        console.log("Normalized ICE servers:", this.iceServers);
        return this.iceServers;
      }

      // 尝试直接解析 urls 字段
      if (data.urls && Array.isArray(data.urls)) {
        this.iceServers = [{ urls: data.urls }];
        console.log("Parsed ICE servers from urls:", this.iceServers);
        return this.iceServers;
      }

      console.warn("No valid ICE servers found in response, using defaults");
      return this.getDefaultIceServers();
    } catch (error) {
      console.error("Error fetching ICE servers:", error);
      return this.getDefaultIceServers();
    }
  }

  /**
   * 标准化 ICE 服务器配置
   */
  normalizeIceServers(servers) {
    if (!Array.isArray(servers)) {
      return this.getDefaultIceServers();
    }

    return servers
      .map((server) => {
        const normalized = {};

        // 处理 urls
        if (server.urls) {
          if (Array.isArray(server.urls)) {
            normalized.urls = server.urls;
          } else if (typeof server.urls === "string") {
            normalized.urls = [server.urls];
          }
        } else if (server.url) {
          normalized.urls = [server.url];
        } else {
          console.warn("ICE server missing URLs:", server);
          return null;
        }

        // 确保至少有一个有效的 URL
        if (!normalized.urls || normalized.urls.length === 0) {
          return null;
        }

        // 添加用户名和凭证（如果有）
        if (server.username !== undefined && server.username !== "") {
          normalized.username = server.username;
        }

        if (server.credential !== undefined && server.credential !== "") {
          normalized.credential = server.credential;
        }

        if (server.credentialType !== undefined) {
          normalized.credentialType = server.credentialType;
        }

        // 检查是否是 TURN 服务器，确保有凭证
        const hasTurn = normalized.urls.some((url) =>
          url.toLowerCase().startsWith("turn:"),
        );

        if (hasTurn) {
          if (!normalized.username) normalized.username = "";
          if (!normalized.credential) normalized.credential = "";
          if (!normalized.credentialType)
            normalized.credentialType = "password";
        }

        return normalized;
      })
      .filter((server) => server !== null);
  }

  /**
   * 获取默认 ICE 服务器配置
   */
  getDefaultIceServers() {
    console.log("Using default ICE servers");
    return [
      {
        urls: ["stun:stun.l.google.com:19302"],
      },
      {
        urls: ["stun:global.stun.twilio.com:3478"],
      },
    ];
  }

  /**
   * 创建并配置 WebRTC PeerConnection
   */
  createPeerConnection(iceServers) {
    const config = {
      iceServers: iceServers,
      iceTransportPolicy: "all", // 允许所有类型的候选
      iceCandidatePoolSize: 10, // 增加候选池大小
      bundlePolicy: "max-bundle",
      rtcpMuxPolicy: "require",
    };

    console.log("Creating PeerConnection with config:", config);

    const pc = new RTCPeerConnection(config);
    this.setupPeerConnectionListeners(pc);

    return pc;
  }

  /**
   * 设置 PeerConnection 事件监听器
   */
  setupPeerConnectionListeners(pc) {
    // 连接状态变化
    pc.onconnectionstatechange = () => {
      console.log(`Connection state: ${pc.connectionState}`);
      this.dispatchEvent(
        new CustomEvent("connectionstatechange", {
          detail: { state: pc.connectionState },
        }),
      );

      switch (pc.connectionState) {
        case "connected":
          console.log("✓ WebRTC connection established");
          this.dispatchEvent(new Event("connected"));
          break;
        case "disconnected":
          console.warn("⚠ WebRTC connection disconnected");
          this.dispatchEvent(new Event("disconnected"));
          break;
        case "failed":
          console.error("✗ WebRTC connection failed");
          this.dispatchEvent(new Event("failed"));
          // 尝试自动恢复
          setTimeout(() => this.attemptRecovery(), 3000);
          break;
        case "closed":
          console.log("WebRTC connection closed");
          this.dispatchEvent(new Event("closed"));
          break;
      }
    };

    // ICE 连接状态变化
    pc.oniceconnectionstatechange = () => {
      console.log(`ICE connection state: ${pc.iceConnectionState}`);
      this.dispatchEvent(
        new CustomEvent("iceconnectionstatechange", {
          detail: { state: pc.iceConnectionState },
        }),
      );

      switch (pc.iceConnectionState) {
        case "failed":
          console.error("ICE connection failed, attempting restart...");
          this.dispatchEvent(new Event("icefailed"));
          this.restartIce();
          break;
        case "disconnected":
          console.warn("ICE connection disconnected");
          this.dispatchEvent(new Event("icedisconnected"));
          break;
        case "connected":
        case "completed":
          console.log("✓ ICE connection established");
          this.dispatchEvent(new Event("iceconnected"));
          break;
      }
    };

    // ICE 收集状态变化
    pc.onicegatheringstatechange = () => {
      console.log(`ICE gathering state: ${pc.iceGatheringState}`);
      this.dispatchEvent(
        new CustomEvent("icegatheringstatechange", {
          detail: { state: pc.iceGatheringState },
        }),
      );
    };

    // ICE 候选收集
    pc.onicecandidate = (event) => {
      if (event.candidate) {
        console.log(`ICE candidate: ${event.candidate.candidate}`);
        this.dispatchEvent(
          new CustomEvent("icecandidate", {
            detail: { candidate: event.candidate },
          }),
        );
      } else {
        console.log("✓ ICE candidate gathering complete");
        this.dispatchEvent(new Event("icegatheringcomplete"));
      }
    };

    // 信令状态变化
    pc.onsignalingstatechange = () => {
      console.log(`Signaling state: ${pc.signalingState}`);
      this.dispatchEvent(
        new CustomEvent("signalingstatechange", {
          detail: { state: pc.signalingState },
        }),
      );
    };

    // 注意：不要在这里设置 ontrack，因为调用方（index.html）已经设置了
    // pc.ontrack = (event) => {
    //   console.log("Track received:", event.track.kind);
    //   this.dispatchEvent(new CustomEvent("track", { detail: event }));
    // };

    // 需要重新协商
    pc.onnegotiationneeded = () => {
      console.log("Negotiation needed");
      this.dispatchEvent(new Event("negotiationneeded"));
    };

    // ICE 候选错误
    pc.onicecandidateerror = (event) => {
      console.warn("ICE candidate error:", event);
      this.dispatchEvent(
        new CustomEvent("icecandidateerror", { detail: event }),
      );
    };
  }

  /**
   * 等待 ICE 候选地址收集完成
   */
  async waitForIceGatheringComplete() {
    if (!this.pc) {
      throw new Error("PeerConnection not available");
    }

    // 如果已经完成，直接返回
    if (this.pc.iceGatheringState === "complete") {
      console.log("ICE gathering already complete");
      return;
    }

    // 等待收集完成
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error("ICE gathering timeout after 30 seconds"));
      }, 30000);

      const onGatheringStateChange = () => {
        console.log(`ICE gathering state: ${this.pc.iceGatheringState}`);
        if (this.pc.iceGatheringState === "complete") {
          clearTimeout(timeout);
          this.pc.removeEventListener(
            "icegatheringstatechange",
            onGatheringStateChange,
          );
          console.log("✓ ICE candidate gathering completed successfully");
          resolve();
        }
      };

      this.pc.addEventListener(
        "icegatheringstatechange",
        onGatheringStateChange,
      );

      // 防止已经完成的情况
      if (this.pc.iceGatheringState === "complete") {
        clearTimeout(timeout);
        this.pc.removeEventListener(
          "icegatheringstatechange",
          onGatheringStateChange,
        );
        resolve();
      }
    });
  }

  /**
   * 查看流
   */
  async view(pc, url, token = null) {
    if (this.pc) {
      console.warn("Already viewing, stopping previous connection");
      await this.stop();
    }

    this.token = token;
    this.pc = pc;

    try {
      // 1. 获取 ICE 服务器配置
      const mediainfoUrl = new URL("/mediainfo", url).toString();
      this.iceServers = await this.fetchIceServers(mediainfoUrl, token);

      // 2. 配置 PeerConnection 的 ICE 服务器
      if (this.iceServers.length > 0) {
        const config = pc.getConfiguration();
        config.iceServers = this.iceServers;
        pc.setConfiguration(config);
        console.log("Set ICE servers from /mediainfo:", this.iceServers);
      }

      // 3. 设置 PeerConnection 事件监听器
      this.setupPeerConnectionListeners(pc);

      // 4. 创建 offer
      const offerOptions = {
        offerToReceiveAudio: true,
        offerToReceiveVideo: true,
        iceRestart: false,
      };

      console.log("Creating offer...");
      const offer = await this.pc.createOffer(offerOptions);

      // 5. 设置本地描述
      await this.pc.setLocalDescription(offer);
      console.log("Local description set");

      // 6. 等待 ICE 候选地址收集完成
      console.log("Waiting for ICE candidate gathering to complete...");
      await this.waitForIceGatheringComplete();

      console.log("ICE gathering complete, final SDP:");
      console.log(pc.localDescription.sdp); // 查看包含候选地址的完整SDP

      // 7. 发送 offer 到服务器
      const headers = {
        "Content-Type": "application/sdp",
      };
      if (token) headers["Authorization"] = "Bearer " + token;

      console.log("Sending offer to:", url);
      const response = await fetch(url, {
        method: "POST",
        body: offer.sdp,
        headers,
      });

      if (!response.ok) {
        if (response.status === 401) {
          throw new Error("UNAUTHORIZED");
        } else {
          throw new Error(`Request rejected with status ${response.status}`);
        }
      }

      if (!response.headers.get("location")) {
        throw new Error("Response missing location header");
      }

      this.resourceURL = new URL(response.headers.get("location"), url);
      console.log("Resource URL:", this.resourceURL.toString());

      // 7. 设置远程描述
      const answer = await response.text();
      console.log("Received answer, setting remote description...");

      await this.pc.setRemoteDescription({ type: "answer", sdp: answer });
      console.log("✓ Remote description set successfully");
      console.log(pc.remoteDescription.sdp); // 查看远程候选地址

      this.dispatchEvent(new Event("viewingstarted"));
      return this.resourceURL;
    } catch (error) {
      console.error("Error in view method:", error);

      // 清理资源
      if (this.pc) {
        this.pc.close();
        this.pc = null;
      }

      this.dispatchEvent(new CustomEvent("error", { detail: error }));
      throw error;
    }
  }

  /**
   * 重启 ICE
   */
  async restartIce() {
    if (!this.pc || this.pc.connectionState === "closed") {
      console.warn(
        "Cannot restart ICE: PeerConnection not available or closed",
      );
      return;
    }

    console.log("Restarting ICE...");

    try {
      // 创建新的 offer 并设置 iceRestart: true
      const offer = await this.pc.createOffer({ iceRestart: true });
      await this.pc.setLocalDescription(offer);

      // 等待 ICE 候选地址收集完成
      console.log("Waiting for ICE gathering to complete during restart...");
      await this.waitForIceGatheringComplete();

      // 发送新的 offer 到服务器
      const headers = {
        "Content-Type": "application/sdp",
      };
      if (this.token) headers["Authorization"] = "Bearer " + this.token;

      const response = await fetch(this.resourceURL, {
        method: "POST",
        body: offer.sdp,
        headers,
      });

      if (!response.ok) {
        throw new Error(`ICE restart failed with status ${response.status}`);
      }

      const answer = await response.text();
      await this.pc.setRemoteDescription({ type: "answer", sdp: answer });

      console.log("✓ ICE restart completed");
      this.dispatchEvent(new Event("icerestarted"));
    } catch (error) {
      console.error("ICE restart failed:", error);
      this.dispatchEvent(new CustomEvent("icerestarterror", { detail: error }));
    }
  }

  /**
   * 尝试恢复连接
   */
  async attemptRecovery() {
    if (!this.pc || this.pc.connectionState !== "failed") {
      return;
    }

    console.log("Attempting to recover connection...");

    try {
      // 先尝试简单的 ICE 重启
      await this.restartIce();

      // 如果还是失败，等待一段时间后尝试完全重新连接
      setTimeout(() => {
        if (this.pc && this.pc.connectionState === "failed") {
          console.log("ICE restart failed, attempting full reconnection...");
          this.dispatchEvent(new Event("reconnectionattempt"));
        }
      }, 5000);
    } catch (error) {
      console.error("Recovery attempt failed:", error);
    }
  }

  /**
   * 停止查看
   */
  async stop() {
    if (!this.pc) {
      console.log("No active connection to stop");
      return;
    }

    console.log("Stopping WHEP client...");

    // 1. 关闭 PeerConnection
    this.pc.close();
    this.pc = null;

    // 2. 发送 DELETE 请求到服务器（如果有 resourceURL）
    if (this.resourceURL) {
      const headers = {};
      if (this.token) headers["Authorization"] = "Bearer " + this.token;

      try {
        const response = await fetch(this.resourceURL, {
          method: "DELETE",
          headers,
        });

        if (!response.ok) {
          console.warn(`DELETE request failed with status: ${response.status}`);
        } else {
          console.log("✓ Resource deleted on server");
        }
      } catch (error) {
        console.error("Failed to send DELETE request:", error);
      }

      this.resourceURL = null;
    }

    this.token = null;
    this.iceServers = [];

    console.log("✓ WHEP client stopped");
    this.dispatchEvent(new Event("stopped"));
  }

  /**
   * 获取连接统计信息
   */
  async getStats() {
    if (!this.pc) {
      return null;
    }

    try {
      const stats = await this.pc.getStats();
      const result = {};

      stats.forEach((report) => {
        result[report.id] = report;
      });

      return result;
    } catch (error) {
      console.error("Error getting stats:", error);
      return null;
    }
  }

  /**
   * 获取当前连接状态
   */
  getConnectionState() {
    return this.pc
      ? {
          connectionState: this.pc.connectionState,
          iceConnectionState: this.pc.iceConnectionState,
          iceGatheringState: this.pc.iceGatheringState,
          signalingState: this.pc.signalingState,
        }
      : null;
  }
}

// 使用方法示例：
/*
// 创建客户端实例
const whepClient = new WHEPClient();

// 监听事件
whepClient.addEventListener('connected', () => {
  console.log('Connected!');
});

whepClient.addEventListener('error', (event) => {
  console.error('Error:', event.detail);
});

whepClient.addEventListener('icefailed', () => {
  console.log('ICE failed, attempting recovery...');
});

// 开始查看
try {
  const resourceUrl = await whepClient.view(
    'https://server.example.com/whep',
    'your-token-here'
  );
  console.log('Viewing started at:', resourceUrl);
} catch (error) {
  console.error('Failed to start viewing:', error);
}

// 停止查看
await whepClient.stop();
*/
