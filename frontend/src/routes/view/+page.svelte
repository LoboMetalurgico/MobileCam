<script lang="ts">
  import { page } from "$app/state";
  import { Role } from "$lib/interfaces/Role";
  import Player from "$lib/player.svelte";
  import { SessionType } from "$lib/protos/common";
  import { ClientToServer, type ServerToClient } from "$lib/protos/viewer";
  import { WRTCManager } from "$lib/WebRTCManager";
  import { WebsocketManager } from "$lib/websocketManager";
  import { onDestroy, onMount } from "svelte";

  let player: Player;
  let connectionId = $state<number>();
  let socket = new WebsocketManager(Role.Viewer, commandHandler);
  const rtcManager = new WRTCManager({
    onRemoteTrack: (track, streams) => {
      if (player.getStream() !== streams[0]) {
        player.setStream(streams[0]);
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
      player.setAspectRatio(
        player.getVideo().videoWidth / player.getVideo().videoHeight,
      );
      player.setZoom(
        Math.max(0, command.updateVideoTransform.videoTransform.zoom),
      );
      player.setRotation(command.updateVideoTransform.videoTransform.rotation);
      player.setIsNative(
        command.updateVideoTransform.videoTransform.zoom === -1,
      );
    }
    if (command.requestRtcAnswer) {
      connectionId = command.requestRtcAnswer.streamerId;
      // create peer connection and send answer back to streamer
      rtcManager.createPeerConnection({
        id: command.requestRtcAnswer.streamerId,
        type: SessionType.SESSION_TYPE_STREAMER,
      });
      rtcManager.setRemoteDescription(
        connectionId,
        JSON.parse(command.requestRtcAnswer.offer),
      );
      const answer = await rtcManager.createStreamAnswer(
        command.requestRtcAnswer.streamerId,
      );
      const reply = ClientToServer.create();
      reply.rtcAnswerResponse = {
        streamerId: command.requestRtcAnswer.streamerId,
        answer: JSON.stringify(answer),
      };
      socket.send(ClientToServer.encode(reply).finish());
    }
    if (command.iceCandidate && command.iceCandidate.session) {
      rtcManager.addICECandidate(
        command.iceCandidate.session.id,
        JSON.parse(command.iceCandidate.candidate),
      );
    }
    if (command.disconnectStreamer) {
      rtcManager.closePeerConnection(connectionId!);
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

<Player bind:this={player} showOverlay />
