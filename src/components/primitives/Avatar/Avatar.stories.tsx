import type { Participant } from '@/ipc/bindings';
import type { Meta } from '@storybook/react';
import { AvatarStack, SubjectAvatar } from './Avatar';

const meta: Meta = {
  title: 'Primitives/Avatar',
};
export default meta;

function mkP(i: number, name: string | null = null): Participant {
  return {
    id: `p${i}`,
    meetingId: 'demo',
    label: `S${i}`,
    name,
    colorClass: `s-color-${i}`,
    wordCount: 0,
    talkPct: 0,
  };
}

export const Single = () => <SubjectAvatar participant={mkP(1, 'Carlos Rivera')} size={36} />;

export const WithoutName = () => <SubjectAvatar participant={mkP(2)} size={36} />;

export const Stack = () => (
  <AvatarStack
    participants={[mkP(1, 'Carlos Rivera'), mkP(2, 'Diego Pereira'), mkP(3, 'Marta López')]}
    size={32}
  />
);

export const StackWithOverflow = () => (
  <AvatarStack
    participants={Array.from({ length: 7 }, (_, i) => mkP(i + 1, `User ${i + 1}`))}
    size={32}
    max={4}
  />
);
