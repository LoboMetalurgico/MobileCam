<script lang="ts">
  import { page } from "$app/state";
  import { Role } from "$lib/interfaces/Role";
  import Player from "$lib/player.svelte";
  import { Session, SessionType } from "$lib/protos/common";
  import { ClientToServer, type ServerToClient } from "$lib/protos/viewer";
  import { WRTCManager } from "$lib/WebRTCManager";
  import { WebsocketManager } from "$lib/websocketManager";
  import { onDestroy, onMount } from "svelte";

  let player: Player;
  let connectionSession = $state<Session>();
  let socket = new WebsocketManager(Role.Viewer, commandHandler);
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

  async function commandHandler(command: ServerToClient) {
    if (
      command.updateVideoTransform &&
      command.updateVideoTransform.videoTransform
    ) {
      player.setZoom(
        Math.max(0, command.updateVideoTransform.videoTransform.zoom),
      );
      player.setRotation(command.updateVideoTransform.videoTransform.rotation);
    }
    if (command.requestRtcAnswer) {
      connectionSession = {
        id: command.requestRtcAnswer.streamerId,
        type: SessionType.SESSION_TYPE_STREAMER,
      };
      // create peer connection and send answer back to streamer
      rtcManager.createPeerConnection(connectionSession);
      rtcManager.setRemoteDescription(
        connectionSession,
        JSON.parse(command.requestRtcAnswer.offer),
      );
      const answer = await rtcManager.createStreamAnswer(connectionSession);
      const reply = ClientToServer.create();
      reply.rtcAnswerResponse = {
        streamerId: command.requestRtcAnswer.streamerId,
        answer: JSON.stringify(answer),
      };
      socket.send(ClientToServer.encode(reply).finish());
    }
    if (command.iceCandidate && command.iceCandidate.session) {
      rtcManager.addICECandidate(
        command.iceCandidate.session,
        JSON.parse(command.iceCandidate.candidate),
      );
    }
    if (command.disconnectStreamer) {
      player.getVideo().srcObject = null;
      rtcManager.closePeerConnection(connectionSession!);
    }
    if (command.updateMute) {
      player.toggleMute(command.updateMute.muted);
    }
  }

  onMount(() => {
    socket.connect({
      protocol: page.url.protocol,
      hostname: page.url.host,
      extraQueries: [["name", page.url.searchParams.get("name")]],
    });
  });

  onDestroy(() => {
    socket.finish();
  });
</script>

<Player
  bind:this={player}
  showOverlay
  onMuteChange={(state) => {
    const msg = ClientToServer.create();
    msg.updateMuteResponse = {
      muted: state,
    };
    socket.send(ClientToServer.encode(msg).finish());
  }}
/>
