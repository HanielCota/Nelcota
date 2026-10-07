// Profile photo dialog.
import type { Messages } from '../index.svelte'

const ptBR = {
  title: 'Foto de perfil',
  description: 'Aparece na barra lateral deste painel. O centro da imagem vira um círculo.',
  reading: 'Lendo a imagem…',
  pick: 'Clique para escolher ou arraste uma imagem aqui',
  remove: 'Remover foto',
  notAnImage: 'Escolha um arquivo de imagem.',
  unreadable: 'Não foi possível ler essa imagem. Tente PNG, JPEG ou WebP.',
  saved: 'Foto atualizada',
  removed: 'Foto removida',
}

const en: Messages<typeof ptBR> = {
  title: 'Profile photo',
  description: "Shows in this panel's sidebar. The centre of the image becomes a circle.",
  reading: 'Reading the image…',
  pick: 'Click to choose or drop an image here',
  remove: 'Remove photo',
  notAnImage: 'Choose an image file.',
  unreadable: 'Could not read that image. Try PNG, JPEG or WebP.',
  saved: 'Photo updated',
  removed: 'Photo removed',
}

export default { 'pt-BR': ptBR, en }
