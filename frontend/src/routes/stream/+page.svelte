<script lang="ts">
  const ICE_CONFIG = {
    iceServers: [
      { urls: "stun:stun.l.google.com:19302" },
      { urls: "stun:stun1.l.google.com:19302" },
    ],
  };

  type Quality = "high" | "medium" | "low";

  const QUALITY_PROFILES = {
    high: { width: 1920, height: 1080, frameRate: 30, bitrate: 8_000_000 },
    medium: { width: 1280, height: 720, frameRate: 24, bitrate: 2_000_000 },
    low: { width: 640, height: 480, frameRate: 15, bitrate: 500_000 },
  };

  let currentQuality = $state<Quality>("high");
  let localStream = $state<MediaStream>();
  let preview: HTMLVideoElement;

  // Hoisted state
  let peerConnections: Record<string, RTCPeerConnection> = {};
  let candidateQueues: Record<string, RTCIceCandidateInit[]> = {}; // Queue for the race condition
  let ws: WebSocket | undefined = undefined;

  let zoomValue = 1;
  let maxZoom = 5;
  let minZoom = 1;
  let nativeZoomSupported = false;
  let wakeLock: WakeLockSentinel | null = null;
  const facingMode = "environment"; 

  function applyEncodingParams(quality: Quality, pc: RTCPeerConnection) {
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
    candidateQueues[viewerSocketId] = []; // Initialize queue for this viewer

    for (const track of localStream.getTracks()) {
      pc.addTrack(track, localStream);
    }
    
    applyEncodingParams(currentQuality, pc);

    pc.onicecandidate = ({ candidate }) => {
      if (candidate) {
        ws?.send(`g:${viewerSocketId}:#${JSON.stringify(candidate)}`);
      }
    };

    pc.onconnectionstatechange = () => {
      if (pc.connectionState !== "failed") return;
      delete peerConnections[viewerSocketId];
      delete candidateQueues[viewerSocketId];
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
      }
    }

    const offer = await pc.createOffer();
    await pc.setLocalDescription(offer);
    ws?.send(`f:${viewerId}:#${JSON.stringify(pc.localDescription)}`);
  }

  function stringToQuality(string: string): Quality {
    if (string === "3") return "low";
    if (string === "2") return "medium";
    return "high";
  }

  async function applyQuality(quality: Quality) {
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
      track
        .applyConstraints({
          advanced: [{ zoom: val } as MediaTrackConstraintSet],
        })
        .catch(() => {
          preview.style.transform = `scale(${val})`;
          nativeZoomSupported = false;
        });
    } else {
      preview.style.transform = `scale(${val})`;
      nativeZoomSupported = false;
    }

    if (!nativeZoomSupported && localStream) {
      const track = localStream.getVideoTracks()[0];
      const settings = track.getSettings();
      ws?.send(
        `j:${JSON.stringify({
          zoom: val,
          rotate: (settings.width ?? 1920) < (settings.height ?? 1080),
        })}`
      );
    }
  }

  let initialPinchDistance = $state(0);
  let pinchStartZoom = $state(1);

  // Manage DOM touch events safely
  $effect(() => {
    if (!preview) return;

    const handleTouchStart = (e: TouchEvent) => {
      if (e.touches.length === 2) {
        initialPinchDistance = Math.hypot(
          e.touches[0].clientX - e.touches[1].clientX,
          e.touches[0].clientY - e.touches[1].clientY
        );
        pinchStartZoom = zoomValue;
      }
    };

    const handleTouchMove = (e: TouchEvent) => {
      if (e.touches.length !== 2) return;
      const dist = Math.hypot(
        e.touches[0].clientX - e.touches[1].clientX,
        e.touches[0].clientY - e.touches[1].clientY
      );
      const scale = dist / initialPinchDistance;
      const newZoom = Math.max(minZoom, Math.min(maxZoom, pinchStartZoom * scale));
      applyZoom(newZoom);
    };

    preview.addEventListener('touchstart', handleTouchStart, { passive: true });
    preview.addEventListener('touchmove', handleTouchMove, { passive: true });

    return () => {
      preview.removeEventListener('touchstart', handleTouchStart);
      preview.removeEventListener('touchmove', handleTouchMove);
    };
  });

  async function acquireWakeLock() {
    try {
      wakeLock = await navigator.wakeLock.request('screen');
      wakeLock.addEventListener('release', () => {
        document.addEventListener('visibilitychange', reacquireWakeLock, { once: true });
      });
    } catch { /* empty */ }
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

      const track = localStream.getVideoTracks()[0];
      const caps = track.getCapabilities?.() || {};
      
      // @ts-expect-error chrome only feature
      if (caps.zoom) {
        nativeZoomSupported = true;
        // @ts-expect-error chrome only feature
        minZoom = caps.zoom.min;
        // @ts-expect-error chrome only feature
        maxZoom = caps.zoom.max;
      }

      // Ensure proper WebSocket protocol
      const wsProtocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
      ws = new WebSocket(`${wsProtocol}//${window.location.host}/ws?role=streamer`);

      ws.addEventListener("error", (err) => console.error("WebSocket Error:", err));
      
      ws.addEventListener("close", () => {
        // window.location.reload();
      });

      ws.addEventListener("message", async (event) => {
        let commandStr: string = event.data.toString();
        const parts = commandStr.split(":");
        const command = parts.shift();
        const params = parts.join(":");
        
        switch (command) {
          case "e": // Viewer requests offer
            await createOffer(params);
            break;
            
          case "h": { // Received ICE candidate from Viewer
            const candidateString = params.split(":");
            const viewerId = candidateString.shift();
            if (!viewerId) return;
            
            const candidate = JSON.parse(candidateString.join(":").slice(1));
            const pc = peerConnections[viewerId];
            
            if (pc && candidate) {
              // QUEUE LOGIC: Only add if remote description is set
              if (pc.remoteDescription && pc.remoteDescription.type) {
                await pc.addIceCandidate(new RTCIceCandidate(candidate));
              } else {
                console.log(`Queueing ICE candidate for ${viewerId}`);
                candidateQueues[viewerId].push(candidate);
              }
            }
            break;
          }

          case "f": { // Received Answer from Viewer
            const candidateString = params.split(":");
            const viewerId = candidateString.shift();
            if (!viewerId) return;

            const sdp = JSON.parse(candidateString.join(":").slice(1));
            const pc = peerConnections[viewerId];
            console.log(`[SIGNALING] Got Answer. Looking for PC with ID: ${viewerId}. Did we find it?`, !!pc);

            if (pc && pc.signalingState !== "stable") {
              await pc.setRemoteDescription(new RTCSessionDescription(sdp));
              
              // QUEUE LOGIC: Flush the queue now that description is set
              const queue = candidateQueues[viewerId] || [];
              while (queue.length > 0) {
                const queuedCandidate = queue.shift();
                if (queuedCandidate) {
                  await pc.addIceCandidate(new RTCIceCandidate(queuedCandidate));
                }
              }
            }
            break;
          }
          
          case "i": // Apply quality change
            await applyQuality(stringToQuality(params.split(":")[1]));
            break;
        }
      });
    } catch (err) {
      console.error(err);
      alert("Could not start camera");
    }
  }
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
    object-fit: cover; /* Added to prevent stretching */
  }

  button {
    position: fixed;
    top: 20px;
    left: 20px;
    padding: 10px 20px;
    z-index: 10;
  }
</style>