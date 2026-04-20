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
  import VerticalSlider from "$lib/VerticalSlider.svelte";
  import { WRTCManager } from "$lib/WebRTCManager";
  import { WebsocketManager } from "$lib/websocketManager";
  import { onDestroy, onMount } from "svelte";

  let player: Player;
  let asyncPrompt: AsyncPrompt;
  let selectItem: HTMLSelectElement;
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
        nativeZoom: boolean;
        battery: number;
        name: string;
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
    nativeZoom?: boolean,
    session?: Session,
  ) {
    streamers[id] = {
      name: name ?? "Transmissor #" + id,
      zoom: zoom ?? 1,
      nativeZoom: nativeZoom ?? true,
      battery: battery ?? 100,
      session,
    };
  }

  function updateStreamer(
    id: number,
    data: {
      name?: string;
      battery?: number;
      zoom?: number;
      nativeZoom?: boolean;
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
        data.nativeZoom,
        data.session,
      );
      return;
    }
    streamer.name = data.name ?? streamer.name;
    streamer.battery = data.battery ?? streamer.battery;
    streamer.zoom = data.zoom ?? streamer.zoom;
    streamer.nativeZoom = data.nativeZoom ?? streamer.nativeZoom;
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

  function createViewer(session: Session, name?: string, streamerId?: number) {
    viewers[session.id] = {
      name: name ?? "Receptor #" + session.id,
      streamerId,
      sessionType: session.type,
    };
  }

  function updateViewer(session: Session, name?: string, streamerId?: number) {
    const viewer = viewers[session.id];
    if (!viewer) {
      createViewer(session, name, streamerId);
      return;
    }
    viewer.name = name ?? viewer.name;
    viewer.streamerId = streamerId;
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
        true,
        {
          id: streamer.id,
          type: SessionType.SESSION_TYPE_STREAMER,
        },
      );
    }
    viewerCount = detail.viewerCount;
    const msg = ClientToServer.create();
    msg.
  }

  socket.addEventListener("ready", onReady);

  function commandHandler(command: ServerToClient) {
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
        command.iceCandidate.session.id,
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
            true,
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
            viewer.name,
            viewer.streamerId,
          );
        else
          createViewer(
            {
              id: viewer.id,
              type: SessionType.SESSION_TYPE_VIEWER,
            },
            viewer.name,
            viewer.streamerId,
          );
      }
      viewerCount = Object.keys(viewers).length;
      waitViewerCallback(command.listViewersResponse.viewers);
    }
    if (command.requestRtcAnswer) {
      console.log("[Command] Request RTC answer");
      const answer = rtcManager.createStreamAnswer(
        command.requestRtcAnswer.streamerId,
      );
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
        nativeZoom: false,
      });
    }
    if (command.updateZoom) {
      // regular zoom
      console.log("[Command] Update zoom");
      updateStreamer(command.updateZoom.streamerId, {
        zoom: command.updateZoom.zoom,
        nativeZoom: true,
      });
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
  <nav class="sidebar"></nav>
  <div class="pages">
    <div class="page homepage"></div>
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
                }}
              >
                <h3 class="streamerName">
                  {streamers[Number(streamerId)].name}
                </h3>
                <p>Bateria: {streamers[Number(streamerId)].battery}%</p>
                {#if currentStreamerBoxSelected !== Number(streamerId)}
                  <p>
                    Zoom: {Math.round(streamers[Number(streamerId)].zoom)}%
                  </p>
                  <p>Quality: Alta</p>
                {/if}
              </button>
              {#if currentStreamerBoxSelected === Number(streamerId)}
                <div class="streamerReceivers">
                  <p class="receiversTitle">Receptores:</p>
                  {#each Object.entries(viewers).filter((item) => item[1].streamerId === currentStreamerBoxSelected) as receiver, recIndex (recIndex)}
                    <div class="receiver">
                      <span class="receiverName">{receiver[1].name}</span>
                      <button
                        class="removeViewerButton"
                        onclick={() => {
                          const msg = ClientToServer.create();
                          msg.updateWatching = {
                            viewerId: Number(receiver[0]),
                          };
                          socket.send(ClientToServer.encode(msg).finish());
                        }}>X</button
                      >
                    </div>
                  {/each}
                  <button
                    class="addViewerButton"
                    onclick={() => {
                      addViewerToStreamer(Number(streamerId));
                    }}>+</button
                  >
                </div>
              {/if}
            </div>
            {#if currentStreamerBoxSelected === Number(streamerId)}
              <div class="streamerRight">
                <div class="qualityModifier">
                  <span class="selectQualityTitle">Qualidade</span>
                  <select
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
  </div>
  <div class="info">
    <div class="preview">
      <Player bind:this={player} showOverlay />
    </div>
    <div class="logs">
      <h2 class="logsBoxTitle">Logs</h2>
      <p class="logText">
        &gt; MobileCam V1.0.0
        <br />
        <br />
        Transmissores: {Object.keys(streamers).length} <br />
        Receptores: {viewerCount}
        <br />
        <br />
        Controller_id: {controllerId ?? "Conectando..."}
      </p>
    </div>
    <img class="logo" alt="Company Branding" src="/image/logo.png" />
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

  .homepage {
    display: none;
  }

  .pageTitle {
    color: var(--accent-color);
    font-size: 2rem;
    font-weight: bolder;
    width: 100%;
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
    justify-content: start;
    align-items: center;
    cursor: pointer;
    .openStreamer & {
      padding-right: 0;
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
  }

  .receiverName {
    width: 100%;
    font-weight: normal;
    font-size: 1.25rem;
    color: var(--accent-color);
  }

  .removeViewerButton {
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
