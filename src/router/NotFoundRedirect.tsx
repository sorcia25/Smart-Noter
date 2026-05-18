import { Navigate } from 'react-router-dom';
import { Paths } from './paths';

export function NotFoundRedirect() {
  return <Navigate to={Paths.Dashboard} replace />;
}
