<script lang="ts">
  import { page } from "$app/state";
  import { Role } from "$lib/interfaces/Role";
  import { toggleFullScreen } from "$lib/utils/phoneUtils";
  import { WRTCManager } from "$lib/WebRTCManager";
  import { WebsocketManager } from "$lib/websocketManager";
  import { onDestroy } from "svelte";
  import Camera from "../../lib/camera.svelte";
  import Screen from "../../lib/screen.svelte";
  import { ClientToServer, type ServerToClient } from "$lib/protos/streamer";
  import type { Session } from "$lib/interfaces/session";

  // svelte-ignore non_reactive_update
  let sourceMedia: Camera | Screen;
  let isCamera = $state(true);
  let isSocketOpen = $state(false);
  const socket = new WebsocketManager(Role.Streamer, commandHandler);
  const rtcManager = new WRTCManager({
    onICECandidate(session: Session, candidate) {
      const reply = ClientToServer.create();
      reply.iceCandidate = {
        session,
        candidate: JSON.stringify(candidate),
      };
      socket.send(ClientToServer.encode(reply).finish());
    },
  });

  async function setStreamingBandwidth(
    maxBitrate: number,
    maxFramerate: number,
  ) {
    for await (const pc of rtcManager.getConnections()) {
      await rtcManager.setStreamBandwidth(
        WRTCManager.keyToSession(pc),
        maxBitrate,
        maxFramerate,
      );
    }
  }

  function onCameraTransform(transform: {
    zoom: number;
    rotation: number;
    isNative: boolean;
  }) {
    const reply = ClientToServer.create();
    if (transform.isNative) {
      const resetVideoTransform = ClientToServer.create();
      resetVideoTransform.requestVideoTransform = {
        videoTransform: {
          zoom: -1,
          rotation: 0,
        },
      };
      socket.send(ClientToServer.encode(resetVideoTransform).finish());
      reply.requestZoom = {
        zoom: transform.zoom,
      };
    } else {
      reply.requestVideoTransform = {
        videoTransform: {
          zoom: transform.zoom,
          rotation: transform.rotation,
        },
      };
    }
    socket.send(ClientToServer.encode(reply).finish());
  }

  async function createOffer(session: Session) {
    const localStream = sourceMedia.getVideoStream();
    if (!localStream) return;
    const pc = rtcManager.createPeerConnection(session);
    if (!pc) return;

    for (const track of localStream.getTracks()) {
      pc.addTrack(track, localStream);
    }

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

    const offer = await rtcManager.createStreamOffer(session);
    const reply = ClientToServer.create();
    reply.rtcOfferResponse = {
      session,
      offer: JSON.stringify(offer),
    };
    socket.send(ClientToServer.encode(reply).finish());
  }

  // @ts-expect-error Chrome-only API
  function reportBattery(battery: BatteryManager) {
    const batteryLevel = Math.round(battery.level * 100);
    const reply = ClientToServer.create();
    reply.updateBatteryLevel = {
      batteryLevel,
    };
    socket.send(ClientToServer.encode(reply).finish());
  }

  async function init() {
    await socket.connect({
      protocol: page.url.protocol,
      hostname: page.url.host,
      extraQueries: [["name", page.url.searchParams.get("name")]],
    });
    await sourceMedia.start();

    // @ts-expect-error Chrome-only API
    if (navigator.getBattery) {
      // @ts-expect-error Chrome-only API
      navigator.getBattery().then((battery) => {
        battery.addEventListener("levelchange", () => {
          reportBattery(battery);
        });
      });
    }
  }

  onDestroy(() => {
    socket.finish();
    isSocketOpen = false;
    rtcManager.finish();
  });

  async function commandHandler(command: ServerToClient) {
    if (command.changeQuality && command.changeQuality.quality) {
      sourceMedia.applyQuality(command.changeQuality.quality);
      const msg = ClientToServer.create();
      msg.changeQualityResponse = {
        quality: command.changeQuality.quality,
      };
      socket.send(ClientToServer.encode(msg).finish());
    }
    if (command.changeZoom) {
      if (isCamera) (sourceMedia as Camera).applyZoom(command.changeZoom.zoom);
    }
    if (command.disconnectPeer && command.disconnectPeer.session) {
      rtcManager.closePeerConnection(command.disconnectPeer.session);
    }
    if (command.iceCandidate && command.iceCandidate.session) {
      rtcManager.addICECandidate(
        command.iceCandidate.session,
        JSON.parse(command.iceCandidate.candidate),
      );
    }
    if (command.requestRtcOffer && command.requestRtcOffer.session) {
      await createOffer(command.requestRtcOffer.session);
    }
    if (command.rtcAnswer && command.rtcAnswer.session) {
      rtcManager.setRemoteDescription(
        command.rtcAnswer.session,
        JSON.parse(command.rtcAnswer.answer),
      );
    }
  }
</script>

{#if !isSocketOpen}
  <div class="selectionScreen">
    <button
      id="start-btn"
      class="selectionButton clickable"
      onclick={() => {
        isSocketOpen = true;
        isCamera = true;
        init();
      }}
    >
      <div class="buttonIcon camera"></div>
      <span class="selectionTitle">Start Camera</span>
    </button>
    <button
      id="start-btn"
      class="selectionButton clickable"
      onclick={() => {
        isSocketOpen = true;
        isCamera = false;
        init();
      }}
    >
      <div class="buttonIcon screen"></div>
      <span class="selectionTitle">Start Screen Share</span>
    </button>
  </div>
{:else}
  <div class="overlay">
    <button
      class="fullscreenBtn clickable"
      onclick={(e) => {
        const state = toggleFullScreen();
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (e.target as any).classList.toggle("isFullscreen", state);
      }}
      aria-label="Tela Cheia"><div class="fullscreenIcon"></div></button
    >
    <img class="branding" alt="Company Logo" src="/logo.png" />
  </div>
  {#if isCamera}
    <Camera
      bind:this={sourceMedia}
      onTransform={onCameraTransform}
      {setStreamingBandwidth}
    />
  {:else}
    <Screen
      bind:this={sourceMedia}
      {setStreamingBandwidth}
      onTransform={onCameraTransform}
    />
  {/if}
{/if}

<style>
  .selectionScreen {
    display: grid;
    grid-template-columns: 2fr 1fr;
    grid-template-rows: 1fr;
    width: 100%;
    height: 100%;
    justify-items: center;
    align-items: center;
    gap: 1rem;
    padding: 1rem;

    @media screen and (orientation: portrait) {
      grid-template-rows: 2fr 1fr;
      grid-template-columns: 1fr;
    }
  }

  .selectionButton {
    cursor: pointer;
    border: 1px solid var(--accent-color);
    background: hsla(from var(--accent-color) h s l / 0.3);
    color: var(--accent-color);
    font-size: 1.5rem;
    padding: 1.5rem 0.75rem;
    border-radius: 0.5rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    width: 100%;
    height: 100%;
  }

  .buttonIcon {
    height: 100%;
    width: 100%;
    background: var(--accent-color);
    mask-size: contain;
    mask-repeat: no-repeat;
    mask-position: center center;
    &.camera {
      mask-image: url("/icons/camera.svg");
    }
    &.screen {
      mask-image: url("/icons/screenshare.svg");
    }
  }

  .selectionTitle {
    font-size: 1.5rem;
    font-weight: bolder;
  }

  .overlay {
    position: absolute;
    width: 100%;
    height: 100%;
    padding: 1rem;
    display: flex;
    flex-direction: row;
    justify-content: space-between;
    align-items: start;
    z-index: 9999;
    pointer-events: none;
  }

  .fullscreenBtn {
    width: 3.5rem;
    height: auto;
    aspect-ratio: 1;
    background: hsla(from var(--accent-color) h s l / 0.3);
    border-radius: 0.5rem;
    border: 1px solid var(--accent-color);
    padding: 0.25rem;
    cursor: pointer;
    position: absolute;
    top: 1rem;
    left: 1rem;
  }

  .fullscreenIcon {
    width: 100%;
    height: 100%;
    background: var(--accent-color);
    mask-image: url("/icons/fullscreenicon.svg");
    mask-size: contain;
  }
  .fullscreenIcon:global(.isFullscreen) {
    mask-image: url("/icons/fullscreenexit.svg");
  }

  .clickable {
    pointer-events: all;
  }

  .branding {
    pointer-events: none;
    opacity: 0.35;
    width: 45dvw;
    height: 45dvh;
    object-fit: contain;
    position: absolute;
    bottom: 2rem;
    right: 2rem;
  }
</style>
