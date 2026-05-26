<script setup>
import { inject } from 'vue'
import ChatContainer from '../components/ChatContainer.vue'
import MessageInput from '../components/MessageInput.vue'
import VoiceAssistant from '../components/VoiceAssistant.vue'

const appActions = inject('appActions')
</script>

<template>
  <div class="chat-page">
    <ChatContainer />
    <MessageInput @send="appActions.sendMessage" @abort="appActions.abortChat" />
    <VoiceAssistant />
  </div>
</template>

<style scoped>
.chat-page {
  min-height: 0;
  flex: 1;
  display: flex;
  flex-direction: column;
  position: relative;
}
</style>
