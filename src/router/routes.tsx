import { lazy } from 'react';
import type { RouteObject } from 'react-router-dom';
import { NotFoundRedirect } from './NotFoundRedirect';
import { Paths } from './paths';

const Dashboard = lazy(() => import('@/features/dashboard/DashboardPage'));
const MeetingsList = lazy(() => import('@/features/meetings-list/MeetingsListPage'));
const PreRecord = lazy(() => import('@/features/pre-record/PreRecordPage'));
const LiveRecording = lazy(() => import('@/features/live-recording/LiveRecordingPage'));
const MeetingDetail = lazy(() => import('@/features/meeting-detail/MeetingDetailPage'));
const Templates = lazy(() => import('@/features/templates/TemplatesPage'));
const Participants = lazy(() => import('@/features/participants/ParticipantsPage'));
const Settings = lazy(() => import('@/features/settings/SettingsPage'));

export const routes: RouteObject[] = [
  { path: Paths.Dashboard, element: <Dashboard /> },
  { path: Paths.Meetings, element: <MeetingsList /> },
  { path: '/meetings/:id', element: <MeetingDetail /> },
  { path: Paths.PreRecord, element: <PreRecord /> },
  { path: '/record/live/:sessionId', element: <LiveRecording /> },
  { path: Paths.Templates, element: <Templates /> },
  { path: Paths.Participants, element: <Participants /> },
  { path: Paths.Settings, element: <Settings /> },
  { path: '*', element: <NotFoundRedirect /> },
];
