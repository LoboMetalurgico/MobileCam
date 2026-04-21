<script lang="ts">
  import { page } from "$app/state";
  import AsyncPrompt from "$lib/asyncPrompt.svelte";
  import { Role } from "$lib/interfaces/Role";
  import type { Session } from "$lib/interfaces/session";
  import Player from "$lib/player.svelte";
  import { Quality, SessionType } from "$lib/protos/common";
  import {
    ClientToServer,
    Ready,
    ViewerSession,
    type ServerToClient,
  } from "$lib/protos/controller";
  import Qrcode from "$lib/qrcode.svelte";
  import VerticalSlider from "$lib/VerticalSlider.svelte";
  import { WRTCManager } from "$lib/WebRTCManager";
  import { WebsocketManager } from "$lib/websocketManager";
  import { onDestroy, onMount } from "svelte";

  let version = $state<string>("1.0.0");

  $effect(() => {
    fetch(`https://${page.url.host}/version`)
      .then((data) => data.text())
      .then((value) => {
        version = value;
      });
  });

  let player: Player;
  let asyncPrompt: AsyncPrompt;
  let selectItem: HTMLSelectElement;
  let inBroadcasterPage = $state<boolean>(false);
  let currentPreviewing = $state<number | null>(null);
  let waitViewerCallback: (
    viewers: ViewerSession[],
  ) => Promise<void> = async () => {};
  const rtcManager = new WRTCManager({
    onRemoteTrack: (track, streams) => {
      if (player.getStream() !== streams[0]) {
        player.setStream(streams[0]);
        player.setAspectRatio(track.getSettings().aspectRatio ?? 16 / 9);
        player
          .play()
          .catch((err) => console.error("Error playing video:", err));
      }
    },

    onICECandidate: (conn, candidate) => {
      const reply = ClientToServer.create();
      reply.iceCandidate = {
        candidate: JSON.stringify(candidate),
        session: conn,
      };
      socket.send(ClientToServer.encode(reply).finish());
    },
  });
  const socket = new WebsocketManager(Role.Controller, commandHandler);
  let currentStreamerBoxSelected = $state<number | null>(null);
  const streamers = $state<
    Record<
      number,
      {
        zoom: number;
        rotation: number;
        nativeZoom: boolean;
        battery: number;
        name: string;
        quality: Quality;
        session?: Session;
      }
    >
  >({});
  const viewers = $state<
    Record<
      number,
      {
        sessionType: SessionType;
        name: string;
        isMute: boolean;
        streamerId?: number;
      }
    >
  >({});
  let controllerId = $state<number>();
  let viewerCount = $state(0);

  function createStreamer(
    id: number,
    name?: string,
    battery?: number,
    zoom?: number,
    rotation?: number,
    nativeZoom?: boolean,
    quality?: Quality,
    session?: Session,
  ) {
    streamers[id] = {
      name: name ?? "Transmissor #" + id,
      zoom: zoom ?? 1,
      rotation: rotation ?? 0,
      nativeZoom: nativeZoom ?? true,
      battery: battery ?? 100,
      quality: quality ?? Quality.QUALITY_HIGH,
      session,
    };
  }

  function updateStreamer(
    id: number,
    data: {
      name?: string;
      battery?: number;
      zoom?: number;
      rotation?: number;
      nativeZoom?: boolean;
      quality?: Quality;
      isMute?: boolean;
      session?: Session;
    },
  ) {
    const streamer = streamers[id];
    if (!streamer) {
      createStreamer(
        id,
        data.name,
        data.battery,
        data.zoom,
        data.rotation,
        data.nativeZoom,
        data.quality,
        data.session,
      );
      return;
    }
    streamer.name = data.name ?? streamer.name;
    streamer.battery = data.battery ?? streamer.battery;
    streamer.zoom = data.zoom ?? streamer.zoom;
    streamer.rotation = data.rotation ?? streamer.rotation;
    streamer.nativeZoom = data.nativeZoom ?? streamer.nativeZoom;
    streamer.quality = data.quality ?? streamer.quality;
    streamer.session = data.session ?? streamer.session;
  }

  function deleteStreamer(id: number) {
    delete streamers[id];
    Object.entries(viewers).forEach(([viewerId, viewer]) => {
      if (viewer.streamerId === id) {
        updateViewer(
          {
            id: Number(viewerId),
            type: viewers[Number(viewerId)].sessionType,
          },
          undefined,
          undefined,
        );
      }
    });
    if (currentStreamerBoxSelected === id) currentStreamerBoxSelected = null;
  }

  function createViewer(
    session: Session,
    isMute: boolean,
    name?: string,
    streamerId?: number,
  ) {
    viewers[session.id] = {
      name: name ?? "Receptor #" + session.id,
      streamerId,
      isMute,
      sessionType: session.type,
    };
  }

  function updateViewer(
    session: Session,
    isMute?: boolean,
    name?: string,
    streamerId?: number | null,
  ) {
    const viewer = viewers[session.id];
    if (!viewer) {
      createViewer(
        session,
        isMute == null ? true : isMute,
        name,
        streamerId == null ? undefined : streamerId,
      );
      return;
    }
    viewer.name = name ?? viewer.name;
    if (streamerId !== undefined)
      viewer.streamerId = streamerId == null ? undefined : streamerId;
    if (isMute != null) viewer.isMute = isMute;
    viewer.sessionType = session.type;
  }

  function deleteViewer(id: number) {
    delete viewers[id];
  }

  function onReady(event: Event) {
    const detail = (event as CustomEvent).detail as Ready;
    controllerId = detail.id;
    for (const streamer of detail.streamers) {
      createStreamer(
        streamer.id,
        streamer.name,
        streamer.batteryLevel,
        streamer.zoom,
        0,
        true,
        Quality.QUALITY_HIGH,
        {
          id: streamer.id,
          type: SessionType.SESSION_TYPE_STREAMER,
        },
      );
    }
    viewerCount = detail.viewerCount;
  }

  socket.addEventListener("ready", onReady);

  async function commandHandler(command: ServerToClient) {
    console.log("[Command] Received command", command);
    if (command.newSession) {
      console.log("[Command] New session");
      if (command.newSession.streamerSession) {
        updateStreamer(command.newSession.streamerSession.id, {
          name: command.newSession.streamerSession.name,
          battery: command.newSession.streamerSession.batteryLevel,
          zoom: command.newSession.streamerSession.zoom,
          session: {
            id: command.newSession.streamerSession.id,
            type: SessionType.SESSION_TYPE_STREAMER,
          },
        });
      }
      if (command.newSession.viewerSession) {
        updateViewer(
          {
            id: command.newSession.viewerSession.id,
            type: SessionType.SESSION_TYPE_VIEWER,
          },
          undefined,
          command.newSession.viewerSession.name,
          command.newSession.viewerSession.streamerId,
        );
        viewerCount += 1;
      }
    }
    if (command.batteryLevel) {
      console.log("[Command] Update battery level");
      updateStreamer(command.batteryLevel.streamerId, {
        battery: command.batteryLevel.batteryLevel,
      });
    }
    if (command.changedWatching) {
      console.log("[Command] Update watching");
      updateViewer(
        {
          id: command.changedWatching.viewerId,
          type: viewers[command.changedWatching.viewerId].sessionType,
        },
        undefined,
        undefined,
        command.changedWatching.streamerId,
      );
    }
    if (command.dropSession && command.dropSession.session) {
      console.log("[Command] Drop session");
      if (
        command.dropSession.session.type === SessionType.SESSION_TYPE_STREAMER
      ) {
        deleteStreamer(command.dropSession.session.id);
      }
      if (
        command.dropSession.session.type === SessionType.SESSION_TYPE_VIEWER
      ) {
        deleteViewer(command.dropSession.session.id);
        viewerCount -= 1;
        if (viewerCount < 0) viewerCount = 0;
      }
    }
    if (command.iceCandidate && command.iceCandidate.session) {
      console.log("[Command] New ICE candidate");
      rtcManager.addICECandidate(
        command.iceCandidate.session,
        JSON.parse(command.iceCandidate.candidate),
      );
    }
    if (command.listStreamersResponse) {
      console.log("[Command] List streamers response");
      for (const streamer of command.listStreamersResponse.streamers) {
        if (streamers[streamer.id])
          updateStreamer(streamer.id, {
            name: streamer.name,
            battery: streamer.batteryLevel,
            zoom: streamer.zoom,
            session: {
              id: streamer.id,
              type: SessionType.SESSION_TYPE_STREAMER,
            },
          });
        else
          createStreamer(
            streamer.id,
            streamer.name,
            streamer.batteryLevel,
            streamer.zoom,
            0,
            true,
            Quality.QUALITY_HIGH,
            { id: streamer.id, type: SessionType.SESSION_TYPE_STREAMER },
          );
      }
    }
    if (command.listViewersResponse) {
      console.log("[Command] List viewers response");
      for (const viewer of command.listViewersResponse.viewers) {
        if (viewers[viewer.id])
          updateViewer(
            {
              id: viewer.id,
              type: SessionType.SESSION_TYPE_VIEWER,
            },
            undefined,
            viewer.name,
            viewer.streamerId,
          );
        else
          createViewer(
            {
              id: viewer.id,
              type: SessionType.SESSION_TYPE_VIEWER,
            },
            true,
            viewer.name,
            viewer.streamerId,
          );
      }
      viewerCount = Object.keys(viewers).length;
      waitViewerCallback(command.listViewersResponse.viewers);
    }
    if (command.requestRtcAnswer) {
      console.log("[Command] Request RTC answer");
      const session = {
        id: command.requestRtcAnswer.streamerId,
        type: SessionType.SESSION_TYPE_STREAMER,
      };
      rtcManager.createPeerConnection(session);
      rtcManager.setRemoteDescription(
        session,
        JSON.parse(command.requestRtcAnswer.offer),
      );
      const answer = await rtcManager.createStreamAnswer(session);
      const reply = ClientToServer.create();
      reply.rtcAnswerResponse = {
        streamerId: command.requestRtcAnswer.streamerId,
        answer: JSON.stringify(answer),
      };
      socket.send(ClientToServer.encode(reply).finish());
    }
    if (
      command.updateVideoTransform &&
      command.updateVideoTransform.videoTransform
    ) {
      console.log("[Command] Update video transform");
      // css fallback zoom
      updateStreamer(command.updateVideoTransform.streamerId, {
        zoom: command.updateVideoTransform.videoTransform.zoom,
        rotation: command.updateVideoTransform.videoTransform.rotation,
        nativeZoom: false,
      });
      player.setRotation(command.updateVideoTransform.videoTransform.rotation);
      player.setZoom(command.updateVideoTransform.videoTransform.zoom);
    }
    if (command.updateZoom) {
      // regular zoom
      console.log("[Command] Update zoom");
      updateStreamer(command.updateZoom.streamerId, {
        zoom: command.updateZoom.zoom,
        nativeZoom: true,
      });
    }
    if (command.changedQuality) {
      updateStreamer(command.changedQuality.streamerId, {
        quality: command.changedQuality.quality,
      });
    }
    if (command.changedMute) {
      updateViewer(
        {
          id: command.changedMute.viewerId,
          type: SessionType.SESSION_TYPE_VIEWER,
        },
        command.changedMute.muted,
      );
    }
  }

  function connectViewerToStreamer(viewerId: number, streamerId: number) {
    const msg = ClientToServer.create();
    msg.updateWatching = {
      viewerId,
      streamerId,
    };
    socket.send(ClientToServer.encode(msg).finish());
  }

  function addViewerToStreamer(streamerId: number) {
    asyncPrompt.updateCloseOnClickOutside(true);
    asyncPrompt.show(
      new Promise((resolve) => {
        waitViewerCallback = async (viewers) => {
          waitViewerCallback = async () => {};
          resolve([viewerAddScreen, [{ streamerId, viewers }]]);
        };
      }),
    );

    const msg = ClientToServer.create();
    msg.listViewers = {};
    socket.send(ClientToServer.encode(msg).finish());
  }

  function startPreviewing(streamerId: number) {
    const msg = ClientToServer.create();
    msg.requestRtcOffer = { streamerId };
    socket.send(ClientToServer.encode(msg).finish());
  }

  function stopPreviewing(streamerId: number) {
    rtcManager.closePeerConnection({
      id: streamerId,
      type: SessionType.SESSION_TYPE_STREAMER,
    });
  }

  onMount(() => {
    socket.connect({
      protocol: window.location.protocol,
      hostname: window.location.host,
    });
  });
  onDestroy(() => {
    socket.removeEventListener("ready", onReady);
    socket.finish();
  });
</script>

{#snippet viewerAddScreen(props: unknown[])}
  <div class="AVSContainer">
    <h2 class="AVSTitle">
      Adicionar Receptor a {streamers[
        (props[0] as { streamerId: number }).streamerId
      ].name}
    </h2>
    <select class="AVSOptions" bind:this={selectItem}>
      {#each (props[0] as { viewers: ViewerSession[] }).viewers.filter((item) => !item.streamerId) as viewer, index (index)}
        <option value={"id=" + viewer.id}
          >{viewer.name ?? "Receptor #" + viewer.id}</option
        >
      {/each}
    </select>
    <button
      class="AVSConfirmButton"
      onclick={() => {
        if (!selectItem.value) return;
        connectViewerToStreamer(
          Number(selectItem.value.split("=")[1]),
          (props[0] as { streamerId: number }).streamerId,
        );
        asyncPrompt.hide();
      }}>Adicionar</button
    >
  </div>
{/snippet}

<div class="controller">
  <AsyncPrompt bind:this={asyncPrompt} />
  <nav class="sidebar">
    <button
      class="sideBarButton"
      title="Inicio"
      onclick={() => {
        inBroadcasterPage = false;
      }}><div class="sidebarIcon homepage"></div></button
    >
    <button
      class="sideBarButton"
      title="Transmissores"
      onclick={() => {
        inBroadcasterPage = true;
      }}><div class="sidebarIcon broadcasters"></div></button
    >
  </nav>
  <div class="pages">
    {#if !inBroadcasterPage}
      <div class="page homepage">
        <div class="qrcodeBox">
          <Qrcode url={`https://${page.url.host}/stream`} />
          <div class="qrcodeboxContent">
            <h2 class="qrcodeboxTitle">Transmissor</h2>
            <p class="qrcodeboxText">
              Escaneie o QRCode ao lado para iniciar um transmissor. Use o
              parâmetro de URL '?name=Valor' para nomear o transmissor.
            </p>
            <a
              class="qrcodeboxLink"
              data-sveltekit-preload-data="off"
              href={`https://${page.url.host}/stream`}
              >{`https://${page.url.host}/stream`}</a
            >
          </div>
        </div>
        <div class="qrcodeBox">
          <Qrcode url={`https://${page.url.host}/view`} />
          <div class="qrcodeboxContent">
            <h2 class="qrcodeboxTitle">Receptor</h2>
            <p class="qrcodeboxText">
              Escaneie o QRCode ao lado para iniciar um receptor. Use o
              parâmetro de URL '?name=Valor' para nomear o receptor.
            </p>
            <a
              class="qrcodeboxLink"
              data-sveltekit-preload-data="off"
              href={`https://${page.url.host}/view`}
              >{`https://${page.url.host}/view`}</a
            >
          </div>
        </div>
      </div>
    {:else}
      <div class="page streamers">
        <h1 class="pageTitle">Transmissores</h1>
        <div class="streamersList">
          {#each Object.keys(streamers) as streamerId, index (index)}
            <div
              class={`streamer ${currentStreamerBoxSelected === Number(streamerId) ? "openStreamer" : ""}`}
            >
              <div class="streamerLeft">
                <button
                  class="streamerHeader"
                  onclick={() => {
                    if (currentStreamerBoxSelected === Number(streamerId))
                      currentStreamerBoxSelected = null;
                    else currentStreamerBoxSelected = Number(streamerId);
                    const msg = ClientToServer.create();
                    msg.listViewers = {
                      streamerId: Number(streamerId),
                    };
                    socket.send(ClientToServer.encode(msg).finish());
                  }}
                >
                  <h3 class="streamerName">
                    <div
                      class={`collapsable ${currentStreamerBoxSelected === Number(streamerId) ? "open" : ""}`}
                    ></div>
                    {streamers[Number(streamerId)].name}
                  </h3>
                  <span class="streamerStats"
                    ><span>{streamers[Number(streamerId)].battery}%</span>
                    <div
                      class="streamerStatsIcon battery"
                      style={`--fill-percent: ${streamers[Number(streamerId)].battery};`}
                    ></div></span
                  >
                  {#if currentStreamerBoxSelected !== Number(streamerId)}
                    <span class="streamerStats">
                      <span
                        >{Math.round(streamers[Number(streamerId)].zoom)}%</span
                      >
                      <div class="streamerStatsIcon zoom"></div>
                    </span>
                    <span class="streamerStats">
                      <span
                        >{streamers[Number(streamerId)].quality ===
                        Quality.QUALITY_LOW
                          ? "Baixa"
                          : streamers[Number(streamerId)].quality ===
                              Quality.QUALITY_MEDIUM
                            ? "Media"
                            : "Alta"}</span
                      >
                      <div class="streamerStatsIcon quality"></div>
                    </span>
                  {/if}
                </button>
                {#if currentStreamerBoxSelected === Number(streamerId)}
                  <div class="streamerReceivers">
                    <p class="receiversTitle">Receptores:</p>
                    {#each Object.entries(viewers).filter((item) => item[1].streamerId === currentStreamerBoxSelected) as receiver, recIndex (recIndex)}
                      <div class="receiver">
                        <button
                          title="mute toggle"
                          class="toggleMuteIcon"
                          onclick={() => {
                            const msg = ClientToServer.create();
                            msg.requestMute = {
                              viewerId: Number(receiver[0]),
                              muted: !receiver[1].isMute,
                            };
                            socket.send(ClientToServer.encode(msg).finish());
                          }}
                          ><div
                            class={`muteIcon ${receiver[1].isMute ? "muted" : ""}`}
                          ></div></button
                        >
                        <span class="receiverName">{receiver[1].name}</span>
                        <button
                          class="removeViewerButton"
                          onclick={() => {
                            const msg = ClientToServer.create();
                            msg.updateWatching = {
                              viewerId: Number(receiver[0]),
                            };
                            socket.send(ClientToServer.encode(msg).finish());
                            updateViewer(
                              {
                                id: Number(receiver[0]),
                                type: receiver[1].sessionType,
                              },
                              undefined,
                              undefined,
                              null,
                            );
                          }}>X</button
                        >
                      </div>
                    {/each}
                    <div class="streamerActions">
                      <button
                        class="addViewerButton"
                        onclick={() => {
                          addViewerToStreamer(Number(streamerId));
                        }}>+</button
                      >
                      {#if currentStreamerBoxSelected === currentPreviewing}
                        <button
                          class="addViewerButton"
                          onclick={() => {
                            stopPreviewing(Number(streamerId));
                          }}>Parar Preview</button
                        >
                      {:else}
                        <button
                          class="addViewerButton"
                          onclick={() => {
                            startPreviewing(Number(streamerId));
                          }}>Preview</button
                        >
                      {/if}
                    </div>
                  </div>
                {/if}
              </div>
              {#if currentStreamerBoxSelected === Number(streamerId)}
                <div class="streamerRight">
                  <div class="qualityModifier">
                    <span class="selectQualityTitle">Qualidade</span>
                    <select
                      value={streamers[Number(streamerId)].quality ===
                      Quality.QUALITY_LOW
                        ? "low"
                        : streamers[Number(streamerId)].quality ===
                            Quality.QUALITY_MEDIUM
                          ? "medium"
                          : "high"}
                      class="selectQuality"
                      onchange={(ev) => {
                        const value = (ev.target as HTMLSelectElement).value;
                        const msgData = ClientToServer.create();
                        msgData.requestChangeQuality = {
                          streamerId: Number(streamerId),
                          quality:
                            value === "high"
                              ? Quality.QUALITY_HIGH
                              : value === "medium"
                                ? Quality.QUALITY_MEDIUM
                                : Quality.QUALITY_LOW,
                        };
                        socket.send(ClientToServer.encode(msgData).finish());
                      }}
                    >
                      <option value="high">Alta</option>
                      <option value="medium">Média</option>
                      <option value="low">Baixa</option>
                    </select>
                  </div>
                  <div class="zoomSliderContainer">
                    <div class="zoomSliderWrapper">
                      <VerticalSlider
                        min={0}
                        max={100}
                        step={1}
                        value={streamers[Number(streamerId)].zoom}
                        onChange={(value) => {
                          const msgData = ClientToServer.create();
                          msgData.requestZoom = {
                            streamerId: Number(streamerId),
                            zoom: value,
                          };
                          socket.send(ClientToServer.encode(msgData).finish());
                        }}
                      />
                    </div>
                  </div>
                </div>
              {/if}
            </div>
          {/each}
        </div>
      </div>
    {/if}
  </div>
  <div class="info">
    <div class="preview">
      <Player bind:this={player} showOverlay />
    </div>
    <div class="logs">
      <h2 class="logsBoxTitle">Logs</h2>
      <p class="logText">
        &gt; MobileCam V{version}
        <br />
        <br />
        Transmissores: {Object.keys(streamers).length} <br />
        Receptores: {viewerCount}
        <br />
        <br />
        Controller_id: {controllerId ?? "Conectando..."}
      </p>
    </div>
    <img class="logo" alt="Company Branding" src="/logo.png" />
  </div>
</div>

<style>
  .controller {
    width: 100%;
    height: 100%;
    display: grid;
    grid-template-columns: 5rem 5fr 2fr;
  }

  .sidebar {
    background: hsla(from var(--accent-color) h s l / 0.3);
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 1rem;
    padding: 0.5rem;
  }

  .sideBarButton {
    width: 100%;
    aspect-ratio: 1;
    background: hsla(from var(--accent-color) h s l / 0.3);
    border: 1px solid var(--accent-color);
    border-radius: 0.5rem;
    padding: 1rem;
    cursor: pointer;
  }

  .sidebarIcon {
    width: 100%;
    height: auto;
    aspect-ratio: 1;
    background: var(--accent-color);
    mask-size: contain;

    &.homepage {
      mask-image: url("/icons/home.svg");
    }
    &.broadcasters {
      mask-image: url("/icons/broadcaster.svg");
    }
  }

  .info {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    padding: 1rem;
    padding-left: 0.5rem;
  }

  .preview {
    width: 100%;
    height: auto;
    aspect-ratio: 16/9;
    border: 1px solid var(--accent-color);
    border-radius: 1rem;
    overflow: clip;
  }

  .logs {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    gap: 0.5rem;
    border: 1px solid var(--accent-color);
    background: var(--black);
    color: var(--accent-color);
  }

  .logsBoxTitle {
    font-size: 1.25rem;
    padding: 0.5rem 1rem;
    background: hsla(from var(--accent-color) h s l / 0.3);
    width: 100%;
    height: fit-content;
  }

  .logText {
    padding-inline: 0.5rem;
    font-family: monospace;
    font-size: 1rem;
  }

  .logo {
    width: 100%;
    object-fit: contain;
  }

  .pages {
    padding: 1rem;
    padding-right: 0;
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    overflow: hidden;
  }

  .page {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    width: 100%;
    height: 100%;
    overflow: hidden;
  }

  .pageTitle {
    color: var(--accent-color);
    font-size: 2rem;
    font-weight: bolder;
    width: 100%;
  }

  .qrcodeBox {
    display: grid;
    grid-template-columns: auto 1fr;
    grid-template-rows: 1fr;
    gap: 1rem;
    color: var(--accent-color);
    background: hsla(from var(--accent-color) h s l / 0.3);
    border: 1px solid var(--accent-color);
    padding: 1rem;
    border-radius: 1rem;
  }

  .qrcode {
    aspect-ratio: 1;
    height: 100%;
    width: auto;
    max-width: 250px;
    object-fit: contain;
  }

  .qrcodeboxContent {
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: 1rem;
  }

  .qrcodeboxTitle {
    font-size: 1.5rem;
    font-weight: bolder;
    letter-spacing: -1px;
  }

  .qrcodeboxLink {
    color: var(--accent-color);
    &:visited {
      color: hsla(from var(--accent-color) calc(h + 25) s l);
    }
  }

  .streamersList {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    width: 100%;
    height: 100%;
    padding-right: 0.5rem;
    overflow-y: scroll;
    scroll-behavior: smooth;
    scrollbar-gutter: stable;

    &::-webkit-scrollbar {
      width: 8px;
    }

    &::-webkit-scrollbar-button {
      display: none;
    }

    &::-webkit-scrollbar-thumb {
      background: var(--accent-color);
      border-radius: 100vw;
    }

    &::-webkit-scrollbar-thumb:hover {
      background: hsla(from var(--accent-color) h s calc(l - 25));
    }

    &::-webkit-scrollbar-track {
      background: hsla(from var(--accent-color) h s l / 0.3);
      border-radius: 100vw;
    }
  }

  @supports (-moz-transform-style: preserve-3d) {
    .streamersList {
      scrollbar-width: thin;
      scrollbar-color: var(--accent-color)
        hsla(from var(--accent-color) h s l / 0.3);
    }
  }

  .streamer {
    width: 100%;
    height: fit-content;
    border-radius: 1rem;
    background: hsla(from var(--accent-color) h s l / 0.3);
    border: 1px solid var(--accent-color);
    overflow: hidden;
    flex-shrink: 0;

    &.openStreamer {
      display: grid;
      grid-template-columns: 1fr 7rem;
      gap: 0.5rem;
    }
  }

  .streamerLeft {
    display: grid;
    grid-template-rows: fit-content 1fr;
  }

  .streamerHeader {
    width: 100%;
    height: 100%;
    padding: 1rem 0.5rem;
    background: transparent;
    border: 0;
    display: flex;
    flex-direction: row;
    gap: 1rem;
    justify-content: start;
    align-items: center;
    cursor: pointer;
    .openStreamer & {
      padding-right: 0;
    }
  }

  .collapsable {
    width: auto;
    height: 2rem;
    aspect-ratio: 1;
    background: var(--accent-color);
    mask-size: contain;
    mask-image: url("/icons/collapsable-open.svg");
    rotate: -90deg;
    transition: rotate 0.5s ease;
    &.open {
      rotate: 0deg;
    }
  }

  .streamerName {
    width: 100%;
    height: fit-content;
    overflow-wrap: anywhere;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--accent-color);
    font-size: 1.75rem;
    font-weight: bolder;
    text-align: left;
    display: flex;
    flex-direction: row;
    gap: 0.5rem;
    align-items: center;
  }

  .streamerStats {
    display: flex;
    flex-direction: row;
    gap: 0.5rem;
    place-content: center;
    color: var(--accent-color);
    font-size: 1.25rem;
    align-items: center;
  }

  .streamerStatsIcon {
    height: 2rem;
    aspect-ratio: 1;
    width: auto;
    mask-size: contain;
    background: var(--accent-color);
    &.battery {
      mask-image: url("/icons/battery-full.svg");
      position: relative;
      background: linear-gradient(
        90deg,
        var(--accent-color) calc((var(--fill-percent) * 0.9) * 1%),
        transparent calc((var(--fill-percent) * 0.9) * 1%)
      );
    }
    &.battery::before {
      content: "";
      position: absolute;
      top: 0;
      left: 0;
      height: 100%;
      width: 100%;
      background: var(--accent-color);
      mask-image: url("/icons/battery.svg");
      mask-size: contain;
    }
    &.zoom {
      mask-image: url("/icons/zoom.svg");
    }
    &.quality {
      mask-image: url("/icons/quality.svg");
    }
  }

  .streamerReceivers {
    width: 100%;
    height: 100%;
    background: var(--black);
    padding: 0.5rem;
    color: var(--white);
    display: flex;
    flex-direction: column;
    gap: 1em;
    border-top-right-radius: 1rem;
  }

  .receiversTitle {
    font-size: 1.25rem;
    font-weight: normal;
    color: var(--accent-color);
  }

  .receiver {
    display: flex;
    flex-direction: row;
    justify-content: start;
    align-items: center;
    padding: 0.5rem 0.25rem 0.5rem 0.5rem;
    border-radius: 0.5rem;
    background: hsla(from var(--accent-color) h s l / 0.3);
    border: 1px solid var(--accent-color);
    gap: 0.5rem;
  }

  .receiverName {
    width: 100%;
    font-weight: normal;
    font-size: 1.25rem;
    color: var(--accent-color);
  }

  .removeViewerButton,
  .toggleMuteIcon {
    border: 0;
    background: transparent;
    height: 100%;
    aspect-ratio: 1;
    color: var(--accent-color);
    background: hsla(from var(--accent-color) h s l / 0.3);
    border-radius: 0.25rem;
    font-size: 1.25rem;
    font-weight: bolder;
    cursor: pointer;
  }

  .muteIcon {
    height: 100%;
    width: auto;
    aspect-ratio: 1;
    background: var(--accent-color);
    mask-size: contain;
    mask-image: url("/icons/unmute.svg");
    &.muted {
      mask-image: url("/icons/mute.svg");
    }
  }

  .streamerActions {
    display: flex;
    gap: 0.5rem;
  }

  .addViewerButton {
    width: 50%;
    margin-right: auto;
    padding: 0.25rem;
    border: 1px solid var(--accent-color);
    background: hsla(from var(--accent-color) h s l / 0.3);
    color: var(--accent-color);
    font-size: 1.25rem;
    font-weight: bolder;
    border-radius: 0.25rem;
    cursor: pointer;
  }

  .AVSContainer {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    place-content: start;
  }

  .AVSTitle {
    font-size: 1.5rem;
    font-weight: bolder;
    color: var(--accent-color);
    word-break: keep-all;
    white-space: pre;
  }

  .AVSOptions {
    background: var(--black);
    border: 1px solid var(--accent-color);
    color: var(--accent-color);
    padding: 0.5rem;
    border-radius: 0.5rem;
    cursor: pointer;
    font-size: 1.25rem;
  }

  .AVSConfirmButton {
    width: fit-content;
    padding: 0.5rem 1rem;
    border: 1px solid var(--accent-color);
    background: hsla(from var(--accent-color) h s l / 0.3);
    color: var(--accent-color);
    font-size: 1.25rem;
    font-weight: bolder;
    border-radius: 0.5rem;
    cursor: pointer;
  }

  .qualityModifier {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    justify-content: start;
  }

  .selectQualityTitle {
    color: var(--accent-color);
    font-size: 1rem;
    font-weight: bolder;
  }

  .selectQuality {
    background: var(--black);
    border: 1px solid var(--accent-color);
    color: var(--accent-color);
    padding: 0.5rem;
    cursor: pointer;
    font-size: 1rem;
  }

  .streamerRight {
    padding-block: 0.5rem;
    padding-right: 0.5rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    justify-content: start;
  }

  .zoomSliderContainer {
    position: relative;
    width: 100%;
    height: 100%;
  }

  .zoomSliderWrapper {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
  }
</style>
