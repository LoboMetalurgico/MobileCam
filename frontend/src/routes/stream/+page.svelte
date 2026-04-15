<script lang="ts">
  const ICE_CONFIG = {
    iceServers: [
      { urls: "stun:stun.l.google.com:19302" },
      { urls: "stun:stun1.l.google.com:19302" },
    ],
  };
  type quality = "high" | "medium" | "low";
  const QUALITY_PROFILES = {
    high: { width: 1920, height: 1080, frameRate: 30, bitrate: 8_000_000 },
    medium: { width: 1280, height: 720, frameRate: 24, bitrate: 2_000_000 },
    low: { width: 640, height: 480, frameRate: 15, bitrate: 500_000 },
  };
  let currentQuality = $state<quality>("high");
  let localStream = $state<MediaStream>();
  const peerConnections = $state<{ [key: string]: RTCPeerConnection }>({}); // viewerSocketId → RTCPeerConnection
  let zoomValue = $state(1);
  let maxZoom = $state(5);
  let minZoom = $state(1);
  let nativeZoomSupported = $state(false);
  let wakeLock = $state<WakeLockSentinel>();
  const facingMode = "environment"; // rear camera by default

  let preview: HTMLVideoElement;

  let socket = $state<WebSocket>();

  function applyEncodingParams(quality: quality, pc: RTCPeerConnection) {
    if (!pc) return;
    const q = QUALITY_PROFILES[quality];
    for (const sender of pc.getSenders()) {
      if (sender.track?.kind !== "video") continue;
      const params = sender.getParameters();
      if (!params.encodings?.length) params.encodings = [{}];
      params.encodings[0].maxBitrate = q.bitrate;
      params.encodings[0].maxFramerate = q.frameRate;
      sender.setParameters(params).catch(() => {});
    }
  }

  async function createPeerConnection(viewerSocketId: string) {
    if (!localStream) return;
    if (peerConnections[viewerSocketId]) {
      peerConnections[viewerSocketId].close();
    }
    const pc = new RTCPeerConnection(ICE_CONFIG);
    peerConnections[viewerSocketId] = pc;

    for (const track of localStream.getTracks()) {
      pc.addTrack(track, localStream);
    }
    applyEncodingParams(currentQuality, pc);

    pc.onicecandidate = ({ candidate }) => {
      if (candidate)
        socket?.send(`g:${viewerSocketId}:${JSON.stringify(candidate)}`);
    };

    pc.onconnectionstatechange = () => {
      if (pc.connectionState !== "failed") return;
      delete peerConnections[viewerSocketId];
      pc.close();
    };
    return pc;
  }

  async function createOffer(viewerId: string) {
    if (!localStream) return;
    const pc = await createPeerConnection(viewerId);
    if (!pc) return;

    const transceivers = pc.getTransceivers();
    const videoTransceiver = transceivers.find(
      (t) => t.sender.track && t.sender.track.kind === "video",
    );

    if (videoTransceiver) {
      const capabilities = RTCRtpReceiver.getCapabilities("video");
      const h264Codecs = (capabilities ?? { codecs: [] }).codecs.filter(
        (codec) => codec.mimeType.toLowerCase() === "video/h264",
      );

      if (h264Codecs.length > 0) {
        videoTransceiver.setCodecPreferences(h264Codecs);
      } else {
        console.warn("H.264 is not supported by this browser.");
        alert("no h264 codec");
      }
    }

    const offer = await pc.createOffer();
    await pc.setLocalDescription(offer);
    socket?.send(`f:${viewerId}:${JSON.stringify(pc.localDescription)}`);
  }

  function stringToQuality(string: string): quality {
    if (string === "3") return "low";
    if (string === "2") return "medium";
    return "high";
  }

  async function applyQuality(quality: quality) {
    if (!QUALITY_PROFILES[quality]) return;
    currentQuality = quality;
    const q = QUALITY_PROFILES[quality];

    if (localStream) {
      const vTrack = localStream.getVideoTracks()[0];
      if (vTrack) {
        await vTrack
          .applyConstraints({
            width: { exact: q.width },
            height: { exact: q.height },
            frameRate: { exact: q.frameRate },
          })
          .catch(() => {});
      }
    }

    for (const pc of Object.values(peerConnections)) {
      applyEncodingParams(quality, pc);
    }
  }

  function applyZoom(val: number) {
    if (nativeZoomSupported && localStream) {
      const track = localStream.getVideoTracks()[0];
      // chrome only zoom, needs as
      track
        .applyConstraints({
          advanced: [{ zoom: val } as MediaTrackConstraintSet],
        })
        .catch(() => {
          // Fallback to CSS zoom if native fails
          preview.style.transform = `scale(${val})`;
          nativeZoomSupported = false;
        });
    } else {
      preview.style.transform = `scale(${val})`;
      nativeZoomSupported = false;
    }

    if (!nativeZoomSupported) {
      const track = localStream
        ? (localStream.getVideoTracks()[0] as unknown as {
            getSettings: () => {
              width: number;
              height: number;
            };
          })
        : {
            getSettings: () => {
              return { width: 1920, height: 1080 };
            },
          };
      socket?.send(
        `j:${JSON.stringify({
          zoom: val,
          rotate: track.getSettings().width < track.getSettings().height,
        })}`,
      );
    }
  }

  let initialPinchDistance = $state(0);
  let pinchStartZoom = $state(1);

  $effect(() => {
    preview.addEventListener('touchstart', e => {
      if (e.touches.length === 2) {
        initialPinchDistance = Math.hypot(
          e.touches[0].clientX - e.touches[1].clientX,
          e.touches[0].clientY - e.touches[1].clientY
        );
        pinchStartZoom = zoomValue;
      }
    }, { passive: true });
  
    preview.addEventListener('touchmove', e => {
      if (e.touches.length !== 2) return;
      const dist = Math.hypot(
        e.touches[0].clientX - e.touches[1].clientX,
        e.touches[0].clientY - e.touches[1].clientY
      );
      const scale = dist / initialPinchDistance;
      const newZoom = Math.max(minZoom, Math.min(maxZoom, pinchStartZoom * scale));
      applyZoom(newZoom);
    }, { passive: true });
  })

  async function acquireWakeLock() {
    try {
      wakeLock = await navigator.wakeLock.request('screen');
      wakeLock.addEventListener('release', () => {
        document.addEventListener('visibilitychange', reacquireWakeLock, { once: true });
      });
    } catch { /* empty */ };
  }

  async function reacquireWakeLock() {
    if (document.visibilityState === 'visible') {
      await acquireWakeLock();
    }
  }

  async function startCamera() {
    try {
      await acquireWakeLock();
      const q = QUALITY_PROFILES[currentQuality];
      localStream = await navigator.mediaDevices.getUserMedia({
        video: {
          aspectRatio: { exact: 16/9 },
          facingMode: { ideal: facingMode },
          width:     { exact: q.width },
          height:    { exact: q.height },
          frameRate: { exact: q.frameRate },
        },
        audio: {
          echoCancellation: false,
          noiseSuppression: false,
          autoGainControl: false,
        }
      });

      preview.srcObject = localStream;

      // Check native zoom support
      const track = localStream.getVideoTracks()[0];
      const caps = track.getCapabilities?.() || {};
      // @ts-expect-error chrome only feature
      if (caps.zoom) {
        nativeZoomSupported = true;
        // @ts-expect-error chrome only feature
        minZoom   = caps.zoom.min;
        // @ts-expect-error chrome only feature
        maxZoom   = caps.zoom.max;
      }

      socket = new WebSocket(`https://${window.location.host}/ws`)
    } catch {
      alert("Could not start camera");
    }
  }

  $effect(() => {
    socket?.addEventListener("error", () => {
      alert(window.location.reload());
    });
  
    socket?.addEventListener("close", () => {
      window.location.reload();
    });
    socket?.addEventListener("message", async (event) => {
      let commandStr: string = event.data.toString();
      const parts = commandStr.split(":");
      const command = parts.shift();
      const params = parts.join(":");
      switch (command) {
        case "e": // Create offer
          await createOffer(params);
          break;
        case "h": {
          // ICe candidate
          const candidateString = params.split(":");
          const viewerId = candidateString.shift();
          if (!viewerId) return;
          const candidate = JSON.parse(candidateString.join(":"));
          const pc = peerConnections[viewerId];
          if (pc && candidate) {
            await pc.addIceCandidate(new RTCIceCandidate(candidate));
          }
          break;
        }
        case "f": {
          // Answer
          const candidateString = params.split(":");
          const viewerId = candidateString.shift();
          if (!viewerId) return;
          const sdp = JSON.parse(candidateString.join(":"));
          const pc = peerConnections[viewerId];
          if (pc && pc.signalingState !== "stable") {
            await pc.setRemoteDescription(new RTCSessionDescription(sdp));
          }
          break;
        }
        case "i":
          await applyQuality(stringToQuality(params.split(":")[1]));
          break;
      }
    });
  });
</script>

<video id="preview" autoplay muted playsinline bind:this={preview}></video>
<button id="start-btn" onclick={startCamera}>Start Camera</button>

<style>
  * {
    padding: 0;
    margin: 0;
    box-sizing: border-box;
  }

  video {
    width: 100vw;
    height: 100vh;
    position: absolute;
    top: 0;
    left: 0;
  }

  button {
    position: fixed;
    top: 0;
    left: 0;
    width: fit-content;
    height: fit-content;
  }
</style>