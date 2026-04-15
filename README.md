# MobileCam
A simple rust app that starts a webapp that lets you use your phone as a camera via wifi for streaming purporses


## Websocket command list
```
a - tell controllers to pull streamers list
b - get streamers list
c - return streamers list to controllers

d - ask tell streamer to create a stream offer to me (create peer to peer connection to viewer) Viewer -> Streamer
e - viewer asked for offer.
f - send offer to viewer
g - streamer sends viewer an ice candidate
h - viewer sends streamer an ice candidate
i - controller tells streamer to change quality preset (low, medium, high)
j - streamer tells all viewers and controllers fallback video transformation (zoom & rotate)
k - streamer disconnects, backend sends all viewers and controllers a streamer disconnect message
```

## Command structure

```
: - separator
%<items separated by ,> - array of items, number is array length
#<string> - JSON string of given length 
```

`a`
`b`
`c:%4,7,9` // 4,7,9 are streamer ids, 3 is array length
`d` // 4 is streamer id
`e:1` // 1 is viewer id
`f:1:#{"type":"offer","sdp":"v=0..."}` // 1 is viewer id from streamer, and Streamer id from viewer
`g:1:#{"candidate":"candidate:..."}` // 1 is viewer id
`h:4:#{"candidate":"candidate:..."}` // 4 is streamer id when sent from viewer, but viewer id when received by streamer
`i:4:1` // 4 is streamer id, high is quality preset, 1 - high, 2 - medium, 3 - low
`j:#{"zoom":1.5,"rotate":90,"x":4.475,"y":2.345}`
`k:4` // 4 is streamer id
