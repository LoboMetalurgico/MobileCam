import { ICE_CONFIG } from "./consts";
import type { Session } from "./interfaces/session";

interface WRTCManagerCallbacks {
  onRemoteTrack?: (track: MediaStreamTrack, streams: readonly MediaStream[]) => void;
  onICECandidate?: (session: Session, candidate: RTCIceCandidateInit) => void;
}

export class WRTCManager {
  private connections: Record<number, RTCPeerConnection> = {};
  private candidateQueues: Record<number, RTCIceCandidateInit[]> = {};
  private Callbacks: WRTCManagerCallbacks;

  constructor(callbacks: WRTCManagerCallbacks) {
    this.Callbacks = callbacks;
  }

  public finish() {
    for (const connectionId in this.connections) {
      this.closePeerConnection(Number(connectionId));
    }
  }

  public createPeerConnection(connection: Session) {
    if (this.connections[connection.id]) {
      this.connections[connection.id].close();
    }

    const pc = new RTCPeerConnection(ICE_CONFIG);
    this.connections[connection.id] = pc;
    this.candidateQueues[connection.id] = [];

    pc.addEventListener('connectionstatechange', () => this.onConnectionState(connection, pc.connectionState));
    pc.addEventListener('track', ({ track, streams }) => this.Callbacks.onRemoteTrack?.(track, streams));
    pc.addEventListener('icecandidate', ({ candidate }) => {
      if (candidate) this.Callbacks.onICECandidate?.(connection, candidate.toJSON());
    });

    return pc;
  }

  private async onConnectionState(connection: Session, state: RTCPeerConnectionState) {
    switch (state) {
      case "closed":
      case "failed":
        this.closePeerConnection(connection.id);
        break;
      case "connected":
        this.flushCandidateQueue(connection.id);
        break;
      default:
        break;
    }
  }

  public closePeerConnection(connectionId: number) {
    const pc = this.connections[connectionId];
    if (!pc) return;
    pc.close();
    delete this.connections[connectionId];
    delete this.candidateQueues[connectionId];
  }

  public async addTrack(connectionId: number, track: MediaStreamTrack) {
    const pc = this.connections[connectionId];
    if (!pc) throw new Error("[AddTrack] PeerConnection not found for ID: " + connectionId);
    pc.addTrack(track);
  }

  public async setStreamBandwidth(
    connectionId: number,
    maxBitrate: number,
    maxFramerate: number,
  ) {
    const peerConnection = this.connections[connectionId];
    if (!peerConnection) throw new Error("[SetStreamBandwidth] PeerConnection not found for ID: " + connectionId);

    for (const sender of peerConnection.getSenders()) {
      if (sender.track?.kind !== "video") continue;
      const params = sender.getParameters();
      if (!params.encodings?.length) params.encodings = [{}];
      params.encodings[0].maxBitrate = maxBitrate;
      params.encodings[0].maxFramerate = maxFramerate;
      params.encodings[0].networkPriority = "high";
      params.encodings[0].active = true;
      params.encodings[0].priority = "high";
      params.encodings[0].scaleResolutionDownBy = 1.0;
      await sender.setParameters(params).catch(() => { });
    }
  }

  public getConnections() {
    return Object.keys(this.connections);
  }

  public getPeerConnection(connectionId: number) {
    return this.connections[connectionId];
  }

  public async addICECandidate(connectionId: number, candidate: RTCIceCandidateInit) {
    const pc = this.connections[connectionId];
    if (!pc || !pc.remoteDescription) {
      this.candidateQueues[connectionId]?.push(candidate);
      return;
    }
    await pc.addIceCandidate(new RTCIceCandidate(candidate)).catch((err) => {
      console.warn(`[AddICECandidate] Failed to add ICE candidate for ID: ${connectionId}`, err);
    });
  }

  public async flushCandidateQueue(connectionId: number) {
    const pc = this.connections[connectionId];
    if (!pc || !pc.remoteDescription) return;
    const queue = this.candidateQueues[connectionId] || [];
    for await (const candidate of queue) {
      await pc.addIceCandidate(new RTCIceCandidate(candidate)).catch(() => { });
    }
    this.candidateQueues[connectionId] = [];
  }

  public async setRemoteDescription(connectionId: number, description: RTCSessionDescriptionInit) {
    const pc = this.connections[connectionId];
    if (!pc || (pc.currentLocalDescription !== null && pc.signalingState === "stable")) throw new Error("[SetRemoteDescription] PeerConnection not found or is already connected for ID: " + connectionId);
    await pc.setRemoteDescription(new RTCSessionDescription(description));
    this.flushCandidateQueue(connectionId);
  }

  public async createStreamOffer(connectionId: number) {
    const pc = this.connections[connectionId];
    if (!pc) throw new Error("[CreateStreamOffer] PeerConnection not found for ID: " + connectionId);
    const offer = await pc.createOffer();
    await pc.setLocalDescription(offer);
    return offer;
  }

  public async createStreamAnswer(connectionId: number) {
    const pc = this.connections[connectionId];
    if (!pc) throw new Error("[CreateStreamAnswer] PeerConnection not found for ID: " + connectionId);
    const answer = await pc.createAnswer();
    await pc.setLocalDescription(answer);
    return answer;
  }
}