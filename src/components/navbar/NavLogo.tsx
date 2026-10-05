import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import LogoIcon from '../../assets/logo.png';

export function NavLogo() {
    const { t } = useTranslation();

    return (
        <Link to="/" draggable="false" className="flex items-center gap-2.5 text-lg font-semibold text-gray-900 dark:text-base-content hover:opacity-90 transition-opacity">
            <img
                src={LogoIcon}
                alt="agy-switch"
                className="w-7 h-7 rounded-lg cursor-pointer active:scale-95 transition-transform shrink-0 shadow-sm"
                draggable="false"
                onError={(e) => {
                    e.currentTarget.src = '/logo.png';
                }}
            />
            <span className="text-nowrap">{t('common.app_name', 'AntiGravity Switch')}</span>
        </Link>
    );
}
